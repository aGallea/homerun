use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

const TRAY_ID: &str = "homerun-tray";

pub fn init(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray/idle.png"))?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("HomeRun")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                crate::window::toggle_tray_panel_window(app);
            }
        })
        .build(app)?;

    Ok(())
}

pub fn update_icon(app: &AppHandle, state: &str) -> Result<(), String> {
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "Tray icon not found".to_string())?;

    let icon_bytes: &[u8] = match state {
        "active" => include_bytes!("../icons/tray/active.png"),
        "error" => include_bytes!("../icons/tray/error.png"),
        "offline" => include_bytes!("../icons/tray/offline.png"),
        _ => include_bytes!("../icons/tray/idle.png"),
    };

    let icon = Image::from_bytes(icon_bytes).map_err(|e| e.to_string())?;
    tray.set_icon(Some(icon)).map_err(|e| e.to_string())?;

    let tooltip = match state {
        "active" => "HomeRun — runners active",
        "error" => "HomeRun — runner error",
        "offline" => "HomeRun — daemon offline",
        _ => "HomeRun — idle",
    };
    tray.set_tooltip(Some(tooltip)).map_err(|e| e.to_string())?;

    Ok(())
}
