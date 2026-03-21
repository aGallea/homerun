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
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::daemon_available,
            commands::list_runners,
            commands::create_runner,
            commands::delete_runner,
            commands::start_runner,
            commands::stop_runner,
            commands::restart_runner,
            commands::auth_status,
            commands::login_with_token,
            commands::logout,
            commands::list_repos,
            commands::get_metrics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
