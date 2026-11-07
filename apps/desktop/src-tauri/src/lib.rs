mod client;
mod commands;
mod tray;
mod window;

use client::DaemonClient;
use tauri::menu::{AboutMetadata, MenuBuilder, MenuItem, SubmenuBuilder};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

pub struct AppState {
    pub client: Mutex<DaemonClient>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = match DaemonClient::default_socket() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("Failed to resolve HomeRun daemon connection: {error}");
            return;
        }
    };
    let sidecar_client = client.clone_connection();
    let event_client = client.clone_connection();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_positioner::init())
        .manage(AppState {
            client: Mutex::new(client),
        })
        .setup(move |app| {
            // -- Build menu --
            let check_updates =
                MenuItem::with_id(app, "check_updates", "View Releases...", true, None::<&str>)?;
            let settings =
                MenuItem::with_id(app, "settings", "Settings...", true, Some("CmdOrCtrl+,"))?;

            let about_metadata = AboutMetadata {
                name: Some("HomeRun".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                copyright: Some("© 2026 HomeRun contributors".into()),
                credits: Some("Manage GitHub Actions self-hosted runners".into()),
                ..Default::default()
            };

            let app_submenu = SubmenuBuilder::new(app, "HomeRun")
                .about(Some(about_metadata))
                .item(&check_updates)
                .separator()
                .item(&settings)
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
                .build()?;

            let edit_submenu = SubmenuBuilder::new(app, "Edit")
                .copy()
                .paste()
                .select_all()
                .build()?;

            let toggle_mini = MenuItem::with_id(
                app,
                "toggle_mini",
                "Toggle Mini View",
                true,
                Some("CmdOrCtrl+Shift+M"),
            )?;

            let window_submenu = SubmenuBuilder::new(app, "Window")
                .minimize()
                .item(&toggle_mini)
                .separator()
                .close_window()
                .build()?;

            let github_item =
                MenuItem::with_id(app, "open_github", "HomeRun on GitHub", true, None::<&str>)?;
            let report_issue = MenuItem::with_id(
                app,
                "report_issue",
                "Report an Issue...",
                true,
                None::<&str>,
            )?;

            let help_submenu = SubmenuBuilder::new(app, "Help")
                .item(&github_item)
                .item(&report_issue)
                .build()?;

            let menu = MenuBuilder::new(app)
                .item(&app_submenu)
                .item(&edit_submenu)
                .item(&window_submenu)
                .item(&help_submenu)
                .build()?;

            app.set_menu(menu)?;

            // -- Handle custom menu events --
            app.on_menu_event(move |app_handle, event| {
                use tauri_plugin_opener::OpenerExt;
                match event.id().as_ref() {
                    "check_updates" => {
                        let _ = app_handle
                            .opener()
                            .open_url("https://github.com/lgg/homerun/releases", None::<&str>);
                    }
                    "toggle_mini" => {
                        let _ = crate::window::toggle_mini_window(app_handle);
                    }
                    "settings" => {
                        let _ = app_handle.emit("navigate", "/settings");
                    }
                    "open_github" => {
                        let _ = app_handle
                            .opener()
                            .open_url("https://github.com/lgg/homerun", None::<&str>);
                    }
                    "report_issue" => {
                        let _ = app_handle.opener().open_url(
                            "https://github.com/lgg/homerun/issues/new/choose",
                            None::<&str>,
                        );
                    }
                    _ => {}
                }
            });

            // -- Spawn daemon sidecar if not running --
            let handle = app.handle().clone();
            let client = sidecar_client.clone_connection();
            tauri::async_runtime::spawn(async move {
                if client.health().await.is_ok() {
                    return;
                }

                use tauri_plugin_shell::ShellExt;
                let sidecar = match handle.shell().sidecar("homerund") {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to find homerund sidecar: {e}");
                        return;
                    }
                };
                match sidecar.spawn() {
                    Ok(_) => eprintln!("Daemon sidecar spawned"),
                    Err(e) => eprintln!("Failed to spawn daemon: {e}"),
                }
            });

            // -- Forward daemon WebSocket events to every React window --
            // Reconnect continuously so a daemon restart does not leave the desktop
            // stuck on polling-only updates.
            let event_handle = app.handle().clone();
            let client = event_client.clone_connection();
            tauri::async_runtime::spawn(async move {
                use futures::StreamExt;
                use tokio_tungstenite::tungstenite::Message;

                loop {
                    if let Ok(mut events) = client.connect_events().await {
                        while let Some(message) = events.next().await {
                            match message {
                                Ok(Message::Text(text)) => {
                                    match serde_json::from_str::<serde_json::Value>(&text) {
                                        Ok(event) => {
                                            let _ = event_handle.emit("runner-event", event);
                                        }
                                        Err(error) => {
                                            eprintln!("Ignoring malformed daemon event: {error}");
                                        }
                                    }
                                }
                                Ok(Message::Close(_)) | Err(_) => break,
                                _ => {}
                            }
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });

            if let Some(main) = app.get_webview_window("main") {
                window::install_main_window_close_handler(&main);
            }

            // -- Initialize system tray --
            if let Err(e) = tray::init(app) {
                eprintln!("Failed to initialize tray: {e}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::daemon_available,
            commands::start_daemon,
            commands::stop_daemon,
            commands::restart_daemon,
            commands::list_runners,
            commands::create_runner,
            commands::update_runner_display_name,
            commands::delete_runner,
            commands::start_runner,
            commands::stop_runner,
            commands::restart_runner,
            commands::auth_status,
            commands::login_with_token,
            commands::logout,
            commands::start_device_flow,
            commands::poll_device_flow,
            commands::list_repos,
            commands::get_metrics,
            commands::docker_status,
            commands::service_status,
            commands::install_service,
            commands::uninstall_service,
            commands::get_runner_logs,
            commands::create_batch,
            commands::start_group,
            commands::stop_group,
            commands::restart_group,
            commands::delete_group,
            commands::scale_group,
            commands::get_preferences,
            commands::update_preferences,
            commands::scan_local,
            commands::scan_remote,
            commands::start_scan,
            commands::cancel_scan,
            commands::get_scan_results,
            commands::get_daemon_logs_recent,
            commands::get_runner_steps,
            commands::get_step_logs,
            commands::get_runner_history,
            commands::rerun_workflow,
            commands::get_run_status,
            commands::clear_runner_history,
            commands::delete_history_entry,
            commands::update_tray_icon,
            commands::toggle_mini_window,
            commands::show_main_window,
            commands::hide_all_windows,
            commands::save_mini_position,
            commands::get_mini_position,
            commands::quit_app,
            commands::send_notification,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
