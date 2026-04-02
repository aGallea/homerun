use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use homerun::client::{AuthStatus, CreateRunnerRequest, DeviceFlowResponse, RunnerInfo};

use crate::SharedState;

pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/auth/status", get(auth_status))
        .route("/auth/device", post(start_device_flow))
        .route("/auth/device/poll", post(poll_device_flow))
        .route("/runners", get(list_runners).post(create_runner))
        .route("/runners/{id}", get(get_runner).delete(delete_runner))
        .route("/runners/{id}/start", post(start_runner))
        .route("/runners/{id}/stop", post(stop_runner))
        .route("/runners/{id}/restart", post(restart_runner))
        .route("/runners/{id}/history", get(get_history))
        .route("/runners/{id}/steps", get(get_steps))
        .route("/repos", get(list_repos))
        .route("/metrics", get(get_metrics))
        .route("/scan/local", post(scan_local))
        .route("/scan/remote", post(scan_remote))
        .with_state(state)
}

async fn health() -> &'static str {
    r#"{"status":"ok","version":"0.5.2-mock"}"#
}

async fn auth_status(State(state): State<SharedState>) -> Json<AuthStatus> {
    Json(state.read().await.auth.clone())
}

async fn start_device_flow(
    State(state): State<SharedState>,
) -> Result<Json<DeviceFlowResponse>, StatusCode> {
    let s = state.read().await;
    match &s.device_flow_response {
        Some(resp) => Ok(Json(resp.clone())),
        None => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Deserialize)]
struct PollRequest {
    #[allow(dead_code)]
    device_code: String,
    #[allow(dead_code)]
    interval: Option<u64>,
}

async fn poll_device_flow(
    State(state): State<SharedState>,
    Json(_body): Json<PollRequest>,
) -> Result<Json<AuthStatus>, StatusCode> {
    let s = state.read().await;
    if s.device_flow_authorized {
        Ok(Json(s.auth.clone()))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn list_runners(State(state): State<SharedState>) -> Json<Vec<RunnerInfo>> {
    Json(state.read().await.runners.clone())
}

async fn get_runner(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<RunnerInfo>, StatusCode> {
    let s = state.read().await;
    s.runners
        .iter()
        .find(|r| r.config.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_runner(
    State(state): State<SharedState>,
    Json(req): Json<CreateRunnerRequest>,
) -> (StatusCode, Json<RunnerInfo>) {
    let id = format!("mock-{}", uuid_simple());
    let parts: Vec<&str> = req.repo_full_name.splitn(2, '/').collect();
    let (owner, name) = if parts.len() == 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (req.repo_full_name.clone(), "unknown".to_string())
    };

    let runner = RunnerInfo {
        config: homerun::client::RunnerConfig {
            id: id.clone(),
            name: req.name.unwrap_or_else(|| format!("runner-{id}")),
            repo_owner: owner,
            repo_name: name,
            labels: req
                .labels
                .unwrap_or_else(|| vec!["self-hosted".to_string()]),
            mode: req.mode.unwrap_or_else(|| "app".to_string()),
            work_dir: std::path::PathBuf::from("/tmp/mock-runner"),
            group_id: None,
        },
        state: "offline".to_string(),
        pid: None,
        uptime_secs: None,
        jobs_completed: 0,
        jobs_failed: 0,
        current_job: None,
        job_context: None,
        job_started_at: None,
        estimated_job_duration_secs: None,
        last_completed_job: None,
    };

    state.write().await.runners.push(runner.clone());
    (StatusCode::CREATED, Json(runner))
}

async fn delete_runner(State(state): State<SharedState>, Path(id): Path<String>) -> StatusCode {
    let mut s = state.write().await;
    let before = s.runners.len();
    s.runners.retain(|r| r.config.id != id);
    if s.runners.len() < before {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn start_runner(State(state): State<SharedState>, Path(id): Path<String>) -> StatusCode {
    let mut s = state.write().await;
    if let Some(r) = s.runners.iter_mut().find(|r| r.config.id == id) {
        r.state = "online".to_string();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn stop_runner(State(state): State<SharedState>, Path(id): Path<String>) -> StatusCode {
    let mut s = state.write().await;
    if let Some(r) = s.runners.iter_mut().find(|r| r.config.id == id) {
        r.state = "offline".to_string();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn restart_runner(State(state): State<SharedState>, Path(id): Path<String>) -> StatusCode {
    let mut s = state.write().await;
    if let Some(r) = s.runners.iter_mut().find(|r| r.config.id == id) {
        r.state = "online".to_string();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn get_history(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Json<Vec<homerun::client::JobHistoryEntry>> {
    let s = state.read().await;
    Json(s.job_history.get(&id).cloned().unwrap_or_default())
}

async fn get_steps(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<homerun::client::StepsResponse>, StatusCode> {
    let s = state.read().await;
    s.steps
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn list_repos(State(state): State<SharedState>) -> Json<Vec<homerun::client::RepoInfo>> {
    Json(state.read().await.repos.clone())
}

async fn get_metrics(
    State(state): State<SharedState>,
) -> Result<Json<homerun::client::MetricsResponse>, StatusCode> {
    let s = state.read().await;
    s.metrics.clone().map(Json).ok_or(StatusCode::NOT_FOUND)
}

async fn scan_local(
    State(state): State<SharedState>,
    Json(_body): Json<serde_json::Value>,
) -> Json<Vec<homerun::client::DiscoveredRepo>> {
    Json(state.read().await.scan_local_results.clone())
}

async fn scan_remote(
    State(state): State<SharedState>,
) -> Json<Vec<homerun::client::DiscoveredRepo>> {
    Json(state.read().await.scan_remote_results.clone())
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{t:x}")
}
