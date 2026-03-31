use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const MINI_LABEL: &str = "mini";
const TRAY_PANEL_LABEL: &str = "tray-panel";
const MINI_WIDTH: f64 = 280.0;
const MINI_HEIGHT: f64 = 80.0;
const TRAY_PANEL_WIDTH: f64 = 300.0;
const TRAY_PANEL_HEIGHT: f64 = 200.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniPosition {
    pub x: f64,
    pub y: f64,
}

/// Toggle the mini always-on-top window. Creates it on first call.
pub fn toggle_mini_window(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(MINI_LABEL) {
        if win.is_visible().unwrap_or(false) {
            win.hide().map_err(|e| e.to_string())?;
            if let Some(main_win) = app.get_webview_window("main") {
                let _ = main_win.show();
                let _ = main_win.set_focus();
            }
        } else {
            win.show().map_err(|e| e.to_string())?;
            win.set_focus().map_err(|e| e.to_string())?;
            if let Some(main_win) = app.get_webview_window("main") {
                let _ = main_win.hide();
            }
        }
        return Ok(());
    }

    let url = WebviewUrl::App("/mini".into());
    let builder = WebviewWindowBuilder::new(app, MINI_LABEL, url)
        .title("HomeRun Mini")
        .inner_size(MINI_WIDTH, MINI_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true);

    let win = builder.build().map_err(|e: tauri::Error| e.to_string())?;

    if let Some(pos) = load_mini_position(app) {
        let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
            pos.x, pos.y,
        )));
    } else if let Ok(Some(monitor)) = win.primary_monitor() {
        let screen = monitor.size();
        let scale = monitor.scale_factor();
        let x = (screen.width as f64 / scale) - MINI_WIDTH - 20.0;
        let y = 40.0;
        let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(x, y)));
    }

    // Hide main window
    if let Some(main_win) = app.get_webview_window("main") {
        let _ = main_win.hide();
    }

    Ok(())
}

/// Hide all windows (main + mini) so only the tray icon remains.
pub fn hide_all_windows(app: &AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    if let Some(mini) = app.get_webview_window(MINI_LABEL) {
        let _ = mini.hide();
    }
    if let Some(tray) = app.get_webview_window(TRAY_PANEL_LABEL) {
        let _ = tray.hide();
    }
    Ok(())
}

/// Show and focus the main window, hide the mini window.
pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(mini) = app.get_webview_window(MINI_LABEL) {
        let _ = mini.hide();
    }
    if let Some(main) = app.get_webview_window("main") {
        main.show().map_err(|e| e.to_string())?;
        main.unminimize().map_err(|e| e.to_string())?;
        main.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Toggle the tray dropdown panel. Position it below the tray icon.
/// `tray_center_x` is the horizontal center of the tray icon (physical pixels).
/// `tray_top_y` is the top edge of the tray icon (physical pixels).
/// `tray_bottom_y` is the bottom edge of the tray icon (physical pixels).
pub fn toggle_tray_panel_window(app: &AppHandle, tray_center_x: i32, tray_top_y: i32, tray_bottom_y: i32) {
    if let Some(win) = app.get_webview_window(TRAY_PANEL_LABEL) {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = position_near_tray(&win, tray_center_x, tray_top_y, tray_bottom_y);
            let _ = win.show();
            let _ = win.set_focus();
        }
        return;
    }

    let url = WebviewUrl::App("/tray".into());
    let builder = WebviewWindowBuilder::new(app, TRAY_PANEL_LABEL, url)
        .title("HomeRun Tray")
        .inner_size(TRAY_PANEL_WIDTH, TRAY_PANEL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .focused(true)
        .visible(false); // start hidden, position first

    if let Ok(win) = builder.build() {
        let _ = position_near_tray(&win, tray_center_x, tray_top_y, tray_bottom_y);
        let _ = win.show();

        // Hide on blur
        let app_handle = app.clone();
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(false) = event {
                if let Some(panel) = app_handle.get_webview_window(TRAY_PANEL_LABEL) {
                    let _ = panel.hide();
                }
            }
        });
    }
}

/// Position the tray panel near the tray icon.
/// On macOS (top menu bar): panel appears below the icon.
/// On Windows (bottom taskbar): panel appears above the icon.
fn position_near_tray(
    win: &tauri::WebviewWindow,
    tray_center_x: i32,
    tray_top_y: i32,
    tray_bottom_y: i32,
) -> Result<(), tauri::Error> {
    let scale = win.scale_factor().unwrap_or(1.0);
    let panel_width = (TRAY_PANEL_WIDTH * scale) as i32;
    let panel_height = (TRAY_PANEL_HEIGHT * scale) as i32;
    let x = tray_center_x - panel_width / 2;

    // Get actual window size in physical pixels (accounts for DPI scaling)
    let actual_height = win
        .outer_size()
        .map(|s| s.height as i32)
        .unwrap_or(panel_height);

    // Determine if tray icon is in the bottom half of the screen.
    // If so, position the panel above the tray icon instead of below.
    let y = if let Ok(Some(monitor)) = win.primary_monitor() {
        let screen_height = monitor.size().height as i32;
        if tray_top_y > screen_height / 2 {
            // Bottom taskbar — panel above the tray icon
            tray_top_y - actual_height
        } else {
            // Top menu bar — panel below the tray icon
            tray_bottom_y
        }
    } else {
        tray_bottom_y
    };

    win.set_position(PhysicalPosition::new(x, y))?;
    Ok(())
}

/// Save mini window position to local app data.
pub fn save_mini_pos(app: &AppHandle, x: f64, y: f64) -> Result<(), String> {
    let path = mini_position_path(app)?;
    let pos = MiniPosition { x, y };
    let json = serde_json::to_string(&pos).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Load mini window position from local app data.
pub fn load_mini_position(app: &AppHandle) -> Option<MiniPosition> {
    let path = mini_position_path(app).ok()?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn mini_position_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("mini_position.json"))
}
