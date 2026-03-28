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
    State(state): State<AppState>,
    Json(body): Json<LocalScanRequest>,
) -> Result<Json<Vec<DiscoveredRepo>>, (StatusCode, String)> {
    let labels = state.config.read().await.preferences.scan_labels.clone();
    let repos = scan_local(&body.path, &labels)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}

pub async fn scan_remote_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<DiscoveredRepo>>, (StatusCode, String)> {
    let token = state.auth.token().await;
    let client = GitHubClient::new(token).map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let labels = state.config.read().await.preferences.scan_labels.clone();
    let repos = scan_remote(&client, &labels)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_scan_local_with_temp_dir_returns_ok() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();
        let body = serde_json::json!({ "path": path }).to_string();

        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/scan/local")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_scan_local_returns_json_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();
        let body = serde_json::json!({ "path": path }).to_string();

        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/scan/local")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json.is_array());
    }

    #[tokio::test]
    async fn test_scan_remote_unauthenticated_returns_401() {
        // No token set → GitHubClient::new(None) → UNAUTHORIZED
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/scan/remote")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_scan_local_missing_path_field_returns_error() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/scan/local")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Missing required `path` field → 422 Unprocessable Entity
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
