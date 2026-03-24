mod client;
mod commands;

use client::DaemonClient;
use tokio::sync::Mutex;

pub struct AppState {
    pub client: Mutex<DaemonClient>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = DaemonClient::default_socket();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            client: Mutex::new(client),
        })
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let client = crate::client::DaemonClient::default_socket();
                if client.health().await.is_ok() {
                    return;
                }

                use tauri_plugin_shell::ShellExt;
                let sidecar = match handle.shell().sidecar("binaries/homerund") {
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
            commands::get_daemon_logs_recent,
            commands::get_runner_steps,
            commands::get_step_logs,
            commands::get_runner_history,
            commands::rerun_workflow,
            commands::clear_runner_history,
            commands::delete_history_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
