use tauri::State;

use crate::client::{
    AuthStatus, BatchCreateResponse, CreateBatchRequest, CreateRunnerRequest, DaemonLogEntry,
    DeviceFlowResponse, DiscoveredRepo, DockerStatusResponse, GroupActionResponse, JobHistoryEntry,
    LogEntry, MetricsResponse, Preferences, RepoInfo, RunStatusResponse, RunnerInfo,
    ScaleGroupResponse, ScanResults, StepLogsResponse, StepsResponse,
};
use crate::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShutdownErrorDisposition {
    ServiceManaged,
    AlreadyStopped,
    Fatal,
}

fn classify_shutdown_error(message: &str, daemon_healthy: bool) -> ShutdownErrorDisposition {
    if message.contains("launchd")
        || message.contains("Uninstall the service")
        || message.contains("auto-start service")
        || message.contains("system service")
    {
        ShutdownErrorDisposition::ServiceManaged
    } else if daemon_healthy {
        ShutdownErrorDisposition::Fatal
    } else {
        ShutdownErrorDisposition::AlreadyStopped
    }
}

async fn daemon_is_healthy(client: &crate::client::DaemonClient) -> bool {
    matches!(
        tokio::time::timeout(std::time::Duration::from_secs(2), client.health()).await,
        Ok(Ok(_))
    )
}

fn remove_stale_socket(client: &crate::client::DaemonClient) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(client.socket_path());
    }
    #[cfg(windows)]
    {
        let _ = client;
    }
}

async fn do_start_daemon(
    app_handle: tauri::AppHandle,
    client: crate::client::DaemonClient,
) -> Result<bool, String> {
    use std::time::Duration;
    use tauri_plugin_shell::ShellExt;

    if client.socket_exists() {
        let check = tokio::time::timeout(std::time::Duration::from_secs(2), client.health()).await;
        if matches!(check, Ok(Ok(_))) {
            return Err("Daemon is already running".to_string());
        }
        remove_stale_socket(&client);
    }

    let sidecar = app_handle
        .shell()
        .sidecar("homerund")
        .map_err(|e| format!("Failed to find sidecar: {e}"))?;

    let (_rx, _child) = sidecar
        .spawn()
        .map_err(|e| format!("Failed to spawn daemon: {e}"))?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if client.health().await.is_ok() {
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

#[tauri::command]
pub async fn start_daemon(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let client = state.client.lock().await.clone_connection();
    do_start_daemon(app_handle, client).await
}

/// Helper: stop the daemon (not a Tauri command — avoids State<> lifetime issues)
async fn do_stop_daemon(client: crate::client::DaemonClient) -> Result<bool, String> {
    let active_runners = match client.shutdown().await {
        Ok(count) => count,
        Err(error) => {
            let message = error.to_string();
            let healthy = daemon_is_healthy(&client).await;
            match classify_shutdown_error(&message, healthy) {
                ShutdownErrorDisposition::ServiceManaged => {
                    let retry_client = client.clone_connection();
                    retry_client.uninstall_service().await.map_err(|error| {
                        format!("Failed to uninstall daemon startup service: {error}")
                    })?;
                    match retry_client.shutdown().await {
                        Ok(count) => count,
                        Err(retry_error) => {
                            let retry_message = retry_error.to_string();
                            let retry_healthy = daemon_is_healthy(&retry_client).await;
                            if classify_shutdown_error(&retry_message, retry_healthy)
                                == ShutdownErrorDisposition::AlreadyStopped
                            {
                                remove_stale_socket(&retry_client);
                                return Ok(true);
                            }
                            return Err(format!(
                                "Failed to stop daemon after uninstalling startup service: {retry_message}"
                            ));
                        }
                    }
                }
                ShutdownErrorDisposition::AlreadyStopped => {
                    remove_stale_socket(&client);
                    return Ok(true);
                }
                ShutdownErrorDisposition::Fatal => {
                    return Err(format!("Failed to stop daemon: {message}"));
                }
            }
        }
    };

    let timeout_secs: u64 = 5 + if active_runners > 0 { 15 } else { 0 };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    loop {
        if !client.socket_exists() {
            return Ok(true);
        }
        if tokio::time::Instant::now() >= deadline {
            if daemon_is_healthy(&client).await {
                return Err("Daemon did not shut down in time and is still responding".to_string());
            }
            remove_stale_socket(&client);
            return Ok(true);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

#[tauri::command]
pub async fn stop_daemon(state: State<'_, AppState>) -> Result<bool, String> {
    let client = state.client.lock().await.clone_connection();
    do_stop_daemon(client).await
}

#[tauri::command]
pub async fn restart_daemon(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let client = state.client.lock().await.clone_connection();
    do_stop_daemon(client.clone_connection()).await?;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    do_start_daemon(app_handle, client).await
}

#[tauri::command]
pub async fn health_check(state: State<'_, AppState>) -> Result<bool, String> {
    // Use a fresh client to avoid mutex contention with other commands
    // that may be hanging when the daemon is down.
    let check_client = state.client.lock().await.clone_connection();
    match tokio::time::timeout(std::time::Duration::from_secs(2), check_client.health()).await {
        Ok(Ok(_)) => Ok(true),
        _ => Ok(false),
    }
}

#[tauri::command]
pub async fn list_runners(state: State<'_, AppState>) -> Result<Vec<RunnerInfo>, String> {
    let client = state.client.lock().await.clone_connection();
    client.list_runners().await
}

#[tauri::command]
pub async fn create_runner(
    state: State<'_, AppState>,
    req: CreateRunnerRequest,
) -> Result<RunnerInfo, String> {
    let client = state.client.lock().await.clone_connection();
    client.create_runner(&req).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_runner_display_name(
    state: State<'_, AppState>,
    id: String,
    display_name: Option<String>,
) -> Result<RunnerInfo, String> {
    let client = state.client.lock().await.clone_connection();
    client
        .update_runner_display_name(&id, display_name.as_deref())
        .await
}

#[tauri::command]
pub async fn delete_runner(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.delete_runner(&id).await
}

#[tauri::command]
pub async fn start_runner(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.start_runner(&id).await
}

#[tauri::command]
pub async fn stop_runner(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.stop_runner(&id).await
}

#[tauri::command]
pub async fn restart_runner(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.restart_runner(&id).await
}

#[tauri::command]
pub async fn auth_status(state: State<'_, AppState>) -> Result<AuthStatus, String> {
    let client = state.client.lock().await.clone_connection();
    client.auth_status().await
}

#[tauri::command]
pub async fn login_with_token(
    state: State<'_, AppState>,
    token: String,
) -> Result<AuthStatus, String> {
    let client = state.client.lock().await.clone_connection();
    client.login_with_token(&token).await
}

#[tauri::command]
pub async fn logout(state: State<'_, AppState>) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.logout().await
}

#[tauri::command]
pub async fn list_repos(state: State<'_, AppState>) -> Result<Vec<RepoInfo>, String> {
    let client = state.client.lock().await.clone_connection();
    client.list_repos().await
}

#[tauri::command]
pub async fn get_metrics(state: State<'_, AppState>) -> Result<MetricsResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.get_metrics().await
}

#[tauri::command]
pub async fn docker_status(state: State<'_, AppState>) -> Result<DockerStatusResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.docker_status().await
}

#[tauri::command]
pub async fn start_device_flow(state: State<'_, AppState>) -> Result<DeviceFlowResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.start_device_flow().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn poll_device_flow(
    state: State<'_, AppState>,
    device_code: String,
    interval: u64,
) -> Result<AuthStatus, String> {
    // Clone connection info, then drop the lock immediately so other commands
    // are not blocked during the long-running poll.
    let poll_client = state.client.lock().await.clone_connection();
    poll_client.poll_device_flow(&device_code, interval).await
}

/// Check whether the daemon socket file exists (fast, no network call).
#[tauri::command]
pub async fn daemon_available(state: State<'_, AppState>) -> Result<bool, String> {
    let client = state.client.lock().await.clone_connection();
    Ok(client.socket_exists())
}

#[tauri::command]
pub async fn service_status(state: State<'_, AppState>) -> Result<bool, String> {
    let client = state.client.lock().await.clone_connection();
    client.service_status().await
}

#[tauri::command]
pub async fn install_service(state: State<'_, AppState>) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.install_service().await
}

#[tauri::command]
pub async fn uninstall_service(state: State<'_, AppState>) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.uninstall_service().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_runner_logs(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<Vec<LogEntry>, String> {
    let client = state.client.lock().await.clone_connection();
    client.get_runner_logs(&runner_id).await
}

#[tauri::command]
pub async fn create_batch(
    state: State<'_, AppState>,
    req: CreateBatchRequest,
) -> Result<BatchCreateResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.create_batch(&req).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn start_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupActionResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.start_group(&group_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn stop_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupActionResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.stop_group(&group_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn restart_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupActionResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.restart_group(&group_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<GroupActionResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.delete_group(&group_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn scale_group(
    state: State<'_, AppState>,
    group_id: String,
    count: u8,
) -> Result<ScaleGroupResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.scale_group(&group_id, count).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_preferences(state: State<'_, AppState>) -> Result<Preferences, String> {
    let client = state.client.lock().await.clone_connection();
    client.get_preferences().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_preferences(
    state: State<'_, AppState>,
    prefs: Preferences,
) -> Result<Preferences, String> {
    let client = state.client.lock().await.clone_connection();
    client.update_preferences(&prefs).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn scan_local(
    state: State<'_, AppState>,
    path: String,
) -> Result<Vec<DiscoveredRepo>, String> {
    let client = state.client.lock().await.clone_connection();
    client.scan_local(&path).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn scan_remote(state: State<'_, AppState>) -> Result<Vec<DiscoveredRepo>, String> {
    let client = state.client.lock().await.clone_connection();
    client.scan_remote().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_runner_steps(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<StepsResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.get_runner_steps(&runner_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_step_logs(
    state: State<'_, AppState>,
    runner_id: String,
    step_number: u16,
) -> Result<StepLogsResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.get_step_logs(&runner_id, step_number).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_runner_history(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<Vec<JobHistoryEntry>, String> {
    let client = state.client.lock().await.clone_connection();
    client.get_runner_history(&runner_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn rerun_workflow(
    state: State<'_, AppState>,
    runner_id: String,
    run_url: String,
) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.rerun_workflow(&runner_id, &run_url).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_run_status(
    state: State<'_, AppState>,
    runner_id: String,
    run_url: String,
) -> Result<RunStatusResponse, String> {
    let client = state.client.lock().await.clone_connection();
    client.get_run_status(&runner_id, &run_url).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn clear_runner_history(
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.clear_runner_history(&runner_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_history_entry(
    state: State<'_, AppState>,
    runner_id: String,
    started_at: String,
) -> Result<(), String> {
    let client = state.client.lock().await.clone_connection();
    client.delete_history_entry(&runner_id, &started_at).await
}

#[tauri::command]
pub async fn get_daemon_logs_recent(
    state: State<'_, AppState>,
    level: Option<String>,
    limit: Option<usize>,
    search: Option<String>,
) -> Result<Vec<DaemonLogEntry>, String> {
    let client = state.client.lock().await.clone_connection();
    client
        .get_daemon_logs_recent(level.as_deref(), limit, search.as_deref())
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn update_tray_icon(app_handle: tauri::AppHandle, state: String) -> Result<(), String> {
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
pub async fn hide_all_windows(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::window::hide_all_windows(&app_handle)
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
pub async fn get_mini_position(app_handle: tauri::AppHandle) -> Result<Option<(f64, f64)>, String> {
    Ok(crate::window::load_mini_position(&app_handle).map(|p| (p.x, p.y)))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn quit_app(app_handle: tauri::AppHandle) -> Result<(), String> {
    crate::window::allow_main_window_close();
    app_handle.exit(0);
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn start_scan(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    workspace_path: Option<String>,
    authenticated: bool,
) -> Result<Vec<String>, String> {
    use tauri::Emitter;
    use uuid::Uuid;

    let base_client = state.client.lock().await.clone_connection();
    let mut scan_ids = Vec::new();

    if let Some(path) = workspace_path.filter(|path| !path.trim().is_empty()) {
        let scan_id = Uuid::new_v4().to_string();
        scan_ids.push(scan_id.clone());
        let app = app_handle.clone();
        let error_app = app_handle.clone();
        let scan_client = base_client.clone_connection();
        tokio::spawn(async move {
            let body = serde_json::json!({ "path": path, "scan_id": scan_id }).to_string();
            let event_scan_id = scan_id.clone();
            let result = scan_client
                .stream_sse("POST", "/scan/local/stream", Some(body), move |data| {
                    let _ = app.emit("scan-progress", data);
                })
                .await;
            if let Err(error) = result {
                let payload = serde_json::json!({
                    "type": "failed",
                    "scan_id": event_scan_id,
                    "scan_type": "local",
                    "message": error,
                })
                .to_string();
                let _ = error_app.emit("scan-progress", payload);
            }
        });
    }

    if authenticated {
        let scan_id = Uuid::new_v4().to_string();
        scan_ids.push(scan_id.clone());
        let app = app_handle.clone();
        let error_app = app_handle.clone();
        let scan_client = base_client.clone_connection();
        tokio::spawn(async move {
            let body = serde_json::json!({ "scan_id": scan_id }).to_string();
            let event_scan_id = scan_id.clone();
            let result = scan_client
                .stream_sse("POST", "/scan/remote/stream", Some(body), move |data| {
                    let _ = app.emit("scan-progress", data);
                })
                .await;
            if let Err(error) = result {
                let payload = serde_json::json!({
                    "type": "failed",
                    "scan_id": event_scan_id,
                    "scan_type": "remote",
                    "message": error,
                })
                .to_string();
                let _ = error_app.emit("scan-progress", payload);
            }
        });
    }

    if scan_ids.is_empty() {
        return Err("Configure a workspace path or authenticate before scanning".to_string());
    }
    Ok(scan_ids)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn cancel_scan(
    state: State<'_, AppState>,
    scan_id: String,
) -> Result<serde_json::Value, String> {
    let client = state.client.lock().await.clone_connection();
    client.cancel_scan(&scan_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_scan_results(state: State<'_, AppState>) -> Result<Option<ScanResults>, String> {
    let client = state.client.lock().await.clone_connection();
    client.get_scan_results().await
}

#[tauri::command(rename_all = "snake_case")]
pub fn send_notification(
    app_handle: tauri::AppHandle,
    title: String,
    body: String,
    icon_path: String,
) -> Result<(), String> {
    use tauri_plugin_notification::NotificationExt;

    // The notification plugin provides the native backend on macOS and Windows.
    // The icon path is retained in the command contract for backwards compatibility;
    // platform notification centers use the packaged application icon.
    let _ = icon_path;
    app_handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("Failed to send notification: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_errors_only_count_as_stopped_when_health_is_gone() {
        assert_eq!(
            classify_shutdown_error("connection refused", false),
            ShutdownErrorDisposition::AlreadyStopped
        );
        assert_eq!(
            classify_shutdown_error("temporary transport failure", true),
            ShutdownErrorDisposition::Fatal
        );
        assert_eq!(
            classify_shutdown_error("Uninstall the service first", true),
            ShutdownErrorDisposition::ServiceManaged
        );
    }
}
