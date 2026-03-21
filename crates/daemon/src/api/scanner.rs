use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use std::path::PathBuf;

use crate::github::GitHubClient;
use crate::scanner::{scan_local, scan_remote, DiscoveredRepo};
use crate::server::AppState;

#[derive(Deserialize)]
pub struct LocalScanRequest {
    pub path: PathBuf,
}

pub async fn scan_local_handler(
    State(_state): State<AppState>,
    Json(body): Json<LocalScanRequest>,
) -> Result<Json<Vec<DiscoveredRepo>>, (StatusCode, String)> {
    let repos = scan_local(&body.path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}

pub async fn scan_remote_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<DiscoveredRepo>>, (StatusCode, String)> {
    let token = state.auth.token().await;
    let client = GitHubClient::new(token).map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let repos = scan_remote(&client)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}
