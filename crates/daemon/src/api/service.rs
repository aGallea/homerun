use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;

use crate::server::AppState;

pub async fn install_service(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let daemon_path =
        std::env::current_exe().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    crate::launchd::install_daemon_service(&daemon_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(
        config = ?state.config,
        "Daemon service installed"
    );

    Ok(Json(json!({ "status": "installed" })))
}

pub async fn uninstall_service(
    State(_state): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    crate::launchd::uninstall_daemon_service()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "status": "uninstalled" })))
}

pub async fn service_status(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let installed = crate::launchd::is_daemon_installed();
    Json(json!({ "installed": installed }))
}
