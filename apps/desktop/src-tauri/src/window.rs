use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    window::Monitor, AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

const MAIN_LABEL: &str = "main";
const MINI_LABEL: &str = "mini";
const TRAY_PANEL_LABEL: &str = "tray-panel";
const MAIN_WIDTH: f64 = 1200.0;
const MAIN_HEIGHT: f64 = 800.0;
const MAIN_MIN_WIDTH: f64 = 480.0;
const MAIN_MIN_HEIGHT: f64 = 400.0;
const MINI_WIDTH: f64 = 280.0;
const MINI_HEIGHT: f64 = 80.0;
const MINI_EDGE_MARGIN_LOGICAL: f64 = 12.0;
const TRAY_PANEL_WIDTH: f64 = 300.0;
const TRAY_PANEL_HEIGHT: f64 = 200.0;

static ALLOW_MAIN_CLOSE: AtomicBool = AtomicBool::new(false);

/// Keep the primary webview alive when the user clicks the title-bar X.
/// Hiding instead of destroying it preserves the UI session and its daemon connections.
pub fn install_main_window_close_handler(window: &tauri::WebviewWindow) {
    let main = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if !ALLOW_MAIN_CLOSE.load(Ordering::SeqCst) {
                api.prevent_close();
                let _ = main.hide();
            }
        }
    });
}

pub fn allow_main_window_close() {
    ALLOW_MAIN_CLOSE.store(true, Ordering::SeqCst);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiniPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl ScreenRect {
    fn right(self) -> i64 {
        self.x as i64 + self.width as i64
    }

    fn bottom(self) -> i64 {
        self.y as i64 + self.height as i64
    }
}

fn work_area_rect(monitor: &Monitor) -> ScreenRect {
    let area = monitor.work_area();
    ScreenRect {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
    }
}

fn window_rect(position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> ScreenRect {
    ScreenRect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }
}

fn overlap_area(a: ScreenRect, b: ScreenRect) -> u64 {
    let left = (a.x as i64).max(b.x as i64);
    let top = (a.y as i64).max(b.y as i64);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());

    if right <= left || bottom <= top {
        return 0;
    }

    ((right - left) as u64) * ((bottom - top) as u64)
}

fn clamp_axis(
    position: i32,
    window_size: u32,
    area_start: i32,
    area_size: u32,
    margin: i32,
) -> i32 {
    let area_start = area_start as i64;
    let area_end = area_start + area_size as i64;
    let margin = margin.max(0) as i64;
    let min = area_start + margin;
    let max = area_end - window_size as i64 - margin;

    if max < min {
        return area_start as i32;
    }

    (position as i64).clamp(min, max) as i32
}

fn clamp_position_to_work_area(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    monitor: &Monitor,
) -> PhysicalPosition<i32> {
    let area = work_area_rect(monitor);
    let margin = (MINI_EDGE_MARGIN_LOGICAL * monitor.scale_factor()).round() as i32;

    PhysicalPosition::new(
        clamp_axis(position.x, size.width, area.x, area.width, margin),
        clamp_axis(position.y, size.height, area.y, area.height, margin),
    )
}

fn default_position_on_monitor(
    size: PhysicalSize<u32>,
    monitor: &Monitor,
) -> PhysicalPosition<i32> {
    let area = work_area_rect(monitor);
    let margin = (MINI_EDGE_MARGIN_LOGICAL * monitor.scale_factor()).round() as i32;
    let desired_x = area.right() - size.width as i64 - margin as i64;
    let desired_y = area.y as i64 + margin as i64;

    clamp_position_to_work_area(
        PhysicalPosition::new(desired_x as i32, desired_y as i32),
        size,
        monitor,
    )
}

/// Keep the mini window fully inside the work area of a connected monitor when
/// it is being activated. Do not call this from move/drag events: valid monitors
/// in a multi-display virtual desktop can use negative coordinates.
///
/// Returns `true` when the window already overlapped a connected monitor, and
/// `false` when it had to be recovered from a completely off-screen position.
fn keep_mini_window_on_screen(win: &WebviewWindow) -> Result<bool, tauri::Error> {
    let position = win.outer_position()?;
    let size = win.outer_size()?;
    let monitors = win.available_monitors()?;

    let mut best_monitor: Option<(Monitor, u64)> = None;
    for monitor in &monitors {
        let area = overlap_area(window_rect(position, size), work_area_rect(monitor));
        if best_monitor
            .as_ref()
            .is_none_or(|(_, best_area)| area > *best_area)
        {
            best_monitor = Some((monitor.clone(), area));
        }
    }

    let had_visible_overlap = best_monitor.as_ref().is_some_and(|(_, area)| *area > 0);

    let target_monitor = if had_visible_overlap {
        best_monitor.map(|(monitor, _)| monitor)
    } else {
        win.primary_monitor()?.or_else(|| monitors.first().cloned())
    };

    let Some(target_monitor) = target_monitor else {
        return Ok(had_visible_overlap);
    };

    let target_position = if had_visible_overlap {
        clamp_position_to_work_area(position, size, &target_monitor)
    } else {
        default_position_on_monitor(size, &target_monitor)
    };

    if target_position != position {
        win.set_position(target_position)?;
    }

    Ok(had_visible_overlap)
}

fn show_mini_window(app: &AppHandle, win: &WebviewWindow) -> Result<(), String> {
    keep_mini_window_on_screen(win).map_err(|e| e.to_string())?;
    win.show().map_err(|e| e.to_string())?;
    win.unminimize().map_err(|e| e.to_string())?;
    win.set_focus().map_err(|e| e.to_string())?;

    // Hide the main window only after the mini window is known to be on-screen
    // and successfully shown. This prevents a failed mini activation from
    // leaving the user with no visible application window.
    if let Some(main_win) = app.get_webview_window(MAIN_LABEL) {
        let _ = main_win.hide();
    }

    Ok(())
}

/// Toggle the mini always-on-top window. Creates it on first call.
pub fn toggle_mini_window(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(MINI_LABEL) {
        if win.is_visible().unwrap_or(false) {
            // If the OS still reports an off-screen window as visible (for
            // example after disconnecting a monitor), recover it instead of
            // toggling back to the main window. One click should make the mini
            // view visible again.
            if !keep_mini_window_on_screen(&win).map_err(|e| e.to_string())? {
                win.show().map_err(|e| e.to_string())?;
                win.unminimize().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
                if let Some(main_win) = app.get_webview_window(MAIN_LABEL) {
                    let _ = main_win.hide();
                }
                return Ok(());
            }

            win.hide().map_err(|e| e.to_string())?;
            if let Some(main_win) = app.get_webview_window(MAIN_LABEL) {
                let _ = main_win.show();
                let _ = main_win.unminimize();
                let _ = main_win.set_focus();
            }
        } else {
            show_mini_window(app, &win)?;
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
        .skip_taskbar(true)
        .visible(false);

    let win = builder.build().map_err(|e: tauri::Error| e.to_string())?;

    // Restore the previous position first, then validate it against the
    // currently connected monitors. Stale positions from a removed/rearranged
    // display are reset to a safe top-right position on the primary monitor.
    if let Some(pos) = load_mini_position(app) {
        let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
            pos.x, pos.y,
        )));
    }

    show_mini_window(app, &win)
}

/// Hide all windows (main + mini) so only the tray icon remains.
pub fn hide_all_windows(app: &AppHandle) -> Result<(), String> {
    if let Some(main) = app.get_webview_window(MAIN_LABEL) {
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

/// Show and focus the main window, recreating it if the user closed it.
pub fn show_main_window(app: &AppHandle) -> Result<(), String> {
    if let Some(mini) = app.get_webview_window(MINI_LABEL) {
        let _ = mini.hide();
    }

    let main = match app.get_webview_window(MAIN_LABEL) {
        Some(main) => main,
        None => {
            let main = WebviewWindowBuilder::new(app, MAIN_LABEL, WebviewUrl::App("/".into()))
                .title("HomeRun")
                .inner_size(MAIN_WIDTH, MAIN_HEIGHT)
                .min_inner_size(MAIN_MIN_WIDTH, MAIN_MIN_HEIGHT)
                .build()
                .map_err(|e: tauri::Error| e.to_string())?;
            install_main_window_close_handler(&main);
            main
        }
    };

    main.show().map_err(|e| e.to_string())?;
    main.unminimize().map_err(|e| e.to_string())?;
    main.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// Toggle the tray dropdown panel. Position it below the tray icon.
/// `tray_center_x` is the horizontal center of the tray icon (physical pixels).
/// `tray_top_y` is the top edge of the tray icon (physical pixels).
/// `tray_bottom_y` is the bottom edge of the tray icon (physical pixels).
pub fn toggle_tray_panel_window(
    app: &AppHandle,
    tray_center_x: i32,
    tray_top_y: i32,
    tray_bottom_y: i32,
) {
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

/// Save the exact position selected by the user. Negative coordinates are valid
/// for monitors placed left of or above the primary display. Stale/off-screen
/// saved positions are repaired only when Mini View is shown again.
pub fn save_mini_pos(app: &AppHandle, x: f64, y: f64) -> Result<(), String> {
    let position = MiniPosition { x, y };
    let path = mini_position_path(app)?;
    let json = serde_json::to_string(&position).map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_area_is_zero_for_fully_offscreen_window() {
        assert_eq!(
            overlap_area(
                ScreenRect {
                    x: -500,
                    y: -500,
                    width: 280,
                    height: 80,
                },
                ScreenRect {
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1040,
                },
            ),
            0
        );
    }

    #[test]
    fn clamp_axis_pulls_negative_position_inside_work_area() {
        assert_eq!(clamp_axis(-500, 280, 0, 1920, 12), 12);
        assert_eq!(clamp_axis(-500, 80, 0, 1040, 12), 12);
    }

    #[test]
    fn clamp_axis_keeps_window_inside_right_and_bottom_edges() {
        assert_eq!(clamp_axis(1900, 280, 0, 1920, 12), 1628);
        assert_eq!(clamp_axis(1000, 80, 0, 1040, 12), 948);
    }

    #[test]
    fn clamp_axis_supports_valid_negative_origin_monitors() {
        assert_eq!(clamp_axis(-1500, 280, -1920, 1920, 12), -1500);
        assert_eq!(clamp_axis(-2500, 280, -1920, 1920, 12), -1908);
    }
}
