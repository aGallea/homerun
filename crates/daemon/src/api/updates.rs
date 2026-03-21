use axum::{extract::State, http::StatusCode, Json};

use crate::server::AppState;
use crate::updater;

#[derive(serde::Serialize)]
pub struct UpdateCheckResponse {
    pub update_available: bool,
    pub current: Option<String>,
    pub latest: Option<String>,
}

pub async fn check_updates(
    State(state): State<AppState>,
) -> Result<Json<UpdateCheckResponse>, (StatusCode, String)> {
    let cache_dir = state.config.cache_dir();
    let current = updater::read_cached_version(&cache_dir);

    let latest = updater::fetch_latest_version()
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;

    let update_available = current.as_deref() != Some(latest.as_str());

    Ok(Json(UpdateCheckResponse {
        update_available,
        current,
        latest: Some(latest),
    }))
}
