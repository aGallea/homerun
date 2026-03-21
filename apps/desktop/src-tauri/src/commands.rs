use tauri::{Emitter, State};

use crate::client::{
    AuthStatus, CreateRunnerRequest, DeviceFlowResponse, MetricsResponse, RepoInfo, RunnerInfo,
};
use crate::AppState;

#[tauri::command]
pub async fn health_check(state: State<'_, AppState>) -> Result<bool, String> {
    let client = state.client.lock().await;
    match client.health().await {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
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
pub async fn start_device_flow(
    state: State<'_, AppState>,
) -> Result<DeviceFlowResponse, String> {
    let client = state.client.lock().await;
    client.start_device_flow().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn poll_device_flow(
    state: State<'_, AppState>,
    device_code: String,
    interval: u64,
) -> Result<AuthStatus, String> {
    let client = state.client.lock().await;
    client.poll_device_flow(&device_code, interval).await
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
pub async fn subscribe_runner_logs(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    runner_id: String,
) -> Result<(), String> {
    // Connect to daemon SSE endpoint and forward log entries as Tauri events
    let socket_path = {
        let client = state.client.lock().await;
        client.socket_path().to_path_buf()
    };

    let rid = runner_id.clone();
    tokio::spawn(async move {
        let stream = match tokio::net::UnixStream::connect(&socket_path).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Failed to connect for log streaming: {e}");
                return;
            }
        };

        // Send HTTP request manually
        let request = format!(
            "GET /runners/{rid}/logs HTTP/1.1\r\n\
             Host: localhost\r\n\
             Accept: text/event-stream\r\n\
             Connection: keep-alive\r\n\r\n"
        );

        let (reader, mut writer) = tokio::io::split(stream);
        if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut writer, request.as_bytes()).await {
            eprintln!("Failed to send SSE request: {e}");
            return;
        }

        // Read SSE lines
        use tokio::io::{AsyncBufReadExt, BufReader};
        let buf_reader = BufReader::new(reader);
        let mut lines = buf_reader.lines();

        // Skip HTTP response headers (until empty line)
        while let Ok(Some(line)) = lines.next_line().await {
            if line.is_empty() {
                break;
            }
        }

        // Forward SSE data lines as Tauri events
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(json) = line.strip_prefix("data: ") {
                let _ = app.emit(&format!("runner-log-{}", rid), json.to_string());
            }
        }
    });

    Ok(())
}
