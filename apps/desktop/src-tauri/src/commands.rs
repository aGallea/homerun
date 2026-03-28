use tauri::State;

use crate::client::{
    AuthStatus, BatchCreateResponse, CreateBatchRequest, CreateRunnerRequest, DaemonLogEntry,
    DeviceFlowResponse, DiscoveredRepo, GroupActionResponse, JobHistoryEntry, LogEntry,
    MetricsResponse, Preferences, RepoInfo, RunnerInfo, ScanResults, ScaleGroupResponse,
    StepLogsResponse, StepsResponse,
};
use crate::AppState;

#[tauri::command]
pub async fn start_daemon(app_handle: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_shell::ShellExt;
    use std::time::Duration;

    // Check if daemon is already running
    let client = crate::client::DaemonClient::default_socket();
    if client.socket_exists() {
        let check = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            client.health(),
        ).await;
        if matches!(check, Ok(Ok(_))) {
            return Err("Daemon is already running".to_string());
        }
        // Stale socket — remove it
        let _ = std::fs::remove_file(client.socket_path());
    }

    // Spawn sidecar
    let sidecar = app_handle
        .shell()
        .sidecar("homerund")
        .map_err(|e| format!("Failed to find sidecar: {e}"))?;

    let (_rx, _child) = sidecar
        .spawn()
        .map_err(|e| format!("Failed to spawn daemon: {e}"))?;

    // Poll until healthy
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let fresh = crate::client::DaemonClient::default_socket();
        if fresh.health().await.is_ok() {
            return Ok(true);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(
                "Daemon failed to start within 5 seconds — check logs at ~/.homerun/logs/"
                    .to_string(),
            );
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Helper: stop the daemon (not a Tauri command — avoids State<> lifetime issues)
async fn do_stop_daemon(socket_path: std::path::PathBuf) -> Result<bool, String> {
    let client = crate::client::DaemonClient::new(socket_path.clone());
    let active_runners = match client.shutdown().await {
        Ok(count) => count,
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("launchd") || msg.contains("Uninstall the service") {
                // Uninstall the launchd service, then retry shutdown
                let client = crate::client::DaemonClient::new(socket_path.clone());
                client.uninstall_service().await.map_err(|e| format!("Failed to uninstall launchd service: {e}"))?;
                // Retry shutdown after uninstalling service
                match client.shutdown().await {
                    Ok(count) => count,
                    Err(e2) => {
                        let _ = std::fs::remove_file(&socket_path);
                        let msg2 = e2.to_string();
                        if !msg2.contains("connect") {
                            return Err(format!("Failed to stop daemon after uninstalling service: {msg2}"));
                        }
                        0
                    }
                }
            } else {
                // Already down — clean up stale socket
                let _ = std::fs::remove_file(&socket_path);
                return Ok(true);
            }
        }
    };
    // Scale timeout: 5s base + 15s if there are active runners (stopped concurrently)
    let timeout_secs: u64 = 5 + if active_runners > 0 { 15 } else { 0 };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        if !socket_path.exists() {
            return Ok(true);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("Daemon did not shut down in time".to_string());
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

#[tauri::command]
pub async fn stop_daemon(state: State<'_, AppState>) -> Result<bool, String> {
    let socket_path = state.client.lock().await.socket_path().to_path_buf();
    do_stop_daemon(socket_path).await
}

#[tauri::command]
pub async fn restart_daemon(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let socket_path = state.client.lock().await.socket_path().to_path_buf();
    let _ = do_stop_daemon(socket_path).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    start_daemon(app_handle).await
}

#[tauri::command]
pub async fn health_check(state: State<'_, AppState>) -> Result<bool, String> {
    // Use a fresh client to avoid mutex contention with other commands
    // that may be hanging when the daemon is down.
    let socket_path = {
        let client = state.client.lock().await;
        client.socket_path().to_path_buf()
    };
    let check_client = crate::client::DaemonClient::new(socket_path);
    match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        check_client.health(),
    )
    .await
    {
        Ok(Ok(_)) => Ok(true),
        _ => Ok(false),
    }
}

#[tauri::command]
pub async fn list_runners(state: State<'_, AppState>) -> Result<Vec<RunnerInfo>, String> {
    let client = state.client.lock().await;
    client.list_runners().await
}

#[tauri::command]
pub async fn create_runner(
    state: State<'_, AppState>,
    req: CreateRunnerRequest,
) -> Result<RunnerInfo, String> {
    let client = state.client.lock().await;
    client.create_runner(&req).await
}

#[tauri::command]
pub async fn delete_runner(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.client.lock().await;
    client.delete_runner(&id).await
}

#[tauri::command]
pub async fn start_runner(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.client.lock().await;
    client.start_runner(&id).await
}

#[tauri::command]
pub async fn stop_runner(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.client.lock().await;
    client.stop_runner(&id).await
}

#[tauri::command]
pub async fn restart_runner(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.client.lock().await;
    client.restart_runner(&id).await
}

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> Result<AuthStatus, String> {
    let client = state.client.lock().await;
    client.auth_status().await
}

#[tauri::command]
pub async fn login_with_token(
    state: State<'_, AppState>,
    token: String,
) -> Result<AuthStatus, String> {
    let client = state.client.lock().await;
    client.login_with_token(&token).await
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    let client = state.client.lock().await;
    client.logout().await
}

#[tauri::command]
pub async fn list_repos(state: State<'_, AppState>) -> Result<Vec<RepoInfo>, String> {
    let client = state.client.lock().await;
    client.list_repos().await
}

#[tauri::command]
pub async fn get_metrics(state: State<'_, AppState>) -> Result<MetricsResponse, String> {
    let client = state.client.lock().await;
    client.get_metrics().await
}

#[tauri::command]
pub async fn start_device_flow(state: State<'_, AppState>) -> Result<DeviceFlowResponse, String> {
    let client = state.client.lock().await;
    client.start_device_flow().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn poll_device_flow(
    state: State<'_, AppState>,
    device_code: String,
    interval: u64,
) -> Result<AuthStatus, String> {
    // Get the socket path, then drop the lock immediately so other commands
    // are not blocked during the long-running poll.
    let socket_path = {
        let client = state.client.lock().await;
        client.socket_path().to_path_buf()
    };
    let poll_client = crate::client::DaemonClient::new(socket_path);
    poll_client.poll_device_flow(&device_code, interval).await
}

/// Check whether the daemon socket file exists (fast, no network call).
#[tauri::command]
pub async fn daemon_available(state: State<'_, AppState>) -> Result<bool, String> {
    let client = state.client.lock().await;
    Ok(client.socket_exists())
}

#[tauri::command]
pub async fn service_status(state: State<'_, AppState>) -> Result<bool, String> {
    let client = state.client.lock().await;
    client.service_status().await
}

#[tauri::command]
pub async fn install_service(state: State<'_, AppState>) -> Result<(), String> {
    let client = state.client.lock().await;
    client.install_service().await
}

#[tauri::command]
pub async fn uninstall_service(state: State<'_, AppState>) -> Result<(), String> {
    let client = state.client.lock().await;
    client.uninstall_service().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_runner_logs(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<Vec<LogEntry>, String> {
    let client = state.client.lock().await;
    client.get_runner_logs(&runner_id).await
}

#[tauri::command]
pub async fn create_batch(
    state: State<'_, AppState>,
    req: CreateBatchRequest,
) -> Result<BatchCreateResponse, String> {
    let client = state.client.lock().await;
    client.create_batch(&req).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn start_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupActionResponse, String> {
    let client = state.client.lock().await;
    client.start_group(&group_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn stop_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupActionResponse, String> {
    let client = state.client.lock().await;
    client.stop_group(&group_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn restart_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupActionResponse, String> {
    let client = state.client.lock().await;
    client.restart_group(&group_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupActionResponse, String> {
    let client = state.client.lock().await;
    client.delete_group(&group_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn scale_group(
    state: State<'_, AppState>,
    group_id: String,
    count: u8,
) -> Result<ScaleGroupResponse, String> {
    let client = state.client.lock().await;
    client.scale_group(&group_id, count).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_preferences(state: State<'_, AppState>) -> Result<Preferences, String> {
    let client = state.client.lock().await;
    client.get_preferences().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_preferences(
    state: State<'_, AppState>,
    prefs: Preferences,
) -> Result<Preferences, String> {
    let client = state.client.lock().await;
    client.update_preferences(&prefs).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn scan_local(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<DiscoveredRepo>, String> {
    let client = state.client.lock().await;
    client.scan_local(&path).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn scan_remote(state: State<'_, AppState>) -> Result<Vec<DiscoveredRepo>, String> {
    let client = state.client.lock().await;
    client.scan_remote().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_runner_steps(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<StepsResponse, String> {
    let client = state.client.lock().await;
    client.get_runner_steps(&runner_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_step_logs(
    state: State<'_, AppState>,
    runner_id: String,
    step_number: u16,
) -> Result<StepLogsResponse, String> {
    let client = state.client.lock().await;
    client.get_step_logs(&runner_id, step_number).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_runner_history(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<Vec<JobHistoryEntry>, String> {
    let client = state.client.lock().await;
    client.get_runner_history(&runner_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn rerun_workflow(
    state: State<'_, AppState>,
    runner_id: String,
    run_url: String,
) -> Result<(), String> {
    let client = state.client.lock().await;
    client.rerun_workflow(&runner_id, &run_url).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn clear_runner_history(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<(), String> {
    let client = state.client.lock().await;
    client.clear_runner_history(&runner_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_history_entry(
    state: State<'_, AppState>,
    runner_id: String,
    started_at: String,
) -> Result<(), String> {
    let client = state.client.lock().await;
    client.delete_history_entry(&runner_id, &started_at).await
}

#[tauri::command]
pub async fn get_daemon_logs_recent(
    state: State<'_, AppState>,
    level: Option<String>,
    limit: Option<usize>,
    search: Option<String>,
) -> Result<Vec<DaemonLogEntry>, String> {
    let client = state.client.lock().await;
    client
        .get_daemon_logs_recent(level.as_deref(), limit, search.as_deref())
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_tray_icon(
    app_handle: tauri::AppHandle,
    state: String,
) -> Result<(), String> {
    crate::tray::update_icon(&app_handle, &state)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn toggle_mini_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::window::toggle_mini_window(&app_handle)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn show_main_window(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::window::show_main_window(&app_handle)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn save_mini_position(
    app_handle: tauri::AppHandle,
    x: f64,
    y: f64,
) -> Result<(), String> {
    crate::window::save_mini_pos(&app_handle, x, y)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_mini_position(
    app_handle: tauri::AppHandle,
) -> Result<Option<(f64, f64)>, String> {
    Ok(crate::window::load_mini_position(&app_handle).map(|p| (p.x, p.y)))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn quit_app(app_handle: tauri::AppHandle) -> Result<(), String> {
    app_handle.exit(0);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn start_scan(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    workspace_path: Option<String>,
    authenticated: bool,
) -> Result<(), String> {
    use tauri::Emitter;

    let client = state.client.lock().await;
    let socket_path = client.socket_path().to_path_buf();
    drop(client);

    if let Some(path) = workspace_path {
        let app = app_handle.clone();
        let sock = socket_path.clone();
        tokio::spawn(async move {
            let client = crate::client::DaemonClient::new(sock);
            let body = serde_json::json!({ "path": path }).to_string();
            if let Ok(text) = client.request("POST", "/scan/local/stream", Some(body)).await {
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        let _ = app.emit("scan-progress", data);
                    }
                }
            }
        });
    }

    if authenticated {
        let app = app_handle.clone();
        let sock = socket_path;
        tokio::spawn(async move {
            let client = crate::client::DaemonClient::new(sock);
            if let Ok(text) = client.request("POST", "/scan/remote/stream", None).await {
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        let _ = app.emit("scan-progress", data);
                    }
                }
            }
        });
    }

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cancel_scan(
    state: State<'_, AppState>,
    scan_id: String,
) -> Result<serde_json::Value, String> {
    let client = state.client.lock().await;
    client.cancel_scan(&scan_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_scan_results(
    state: State<'_, AppState>,
) -> Result<Option<ScanResults>, String> {
    let client = state.client.lock().await;
    client.get_scan_results().await
}

#[tauri::command(rename_all = "snake_case")]
pub fn send_notification(title: String, body: String, icon_path: String) -> Result<(), String> {
    mac_notification_sys::Notification::new()
        .title(&title)
        .message(&body)
        .app_icon(&icon_path)
        .send()
        .map(|_| ())
        .map_err(|e| format!("Failed to send notification: {e}"))
}
