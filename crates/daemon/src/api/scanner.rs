use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};
use futures::stream::StreamExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;

use crate::github::GitHubClient;
use crate::scanner::persistence::{self, ScanResults};
use crate::scanner::{scan_local, scan_local_with_progress, scan_remote, DiscoveredRepo};
use crate::server::AppState;

#[derive(Clone, Default)]
pub struct ScanState {
    active_scans: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl ScanState {
    pub fn new() -> Self {
        Self {
            active_scans: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, scan_id: String, cancel: CancellationToken) {
        self.active_scans.lock().await.insert(scan_id, cancel);
    }

    pub async fn cancel(&self, scan_id: &str) -> bool {
        if let Some(token) = self.active_scans.lock().await.remove(scan_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn remove(&self, scan_id: &str) {
        self.active_scans.lock().await.remove(scan_id);
    }
}

#[derive(Deserialize)]
pub struct LocalScanRequest {
    pub path: PathBuf,
    pub labels: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct RemoteScanRequest {
    pub labels: Option<Vec<String>>,
}

pub async fn scan_local_handler(
    State(state): State<AppState>,
    Json(body): Json<LocalScanRequest>,
) -> Result<Json<Vec<DiscoveredRepo>>, (StatusCode, String)> {
    let labels = match body.labels {
        Some(l) if !l.is_empty() => l,
        _ => state.config.read().await.preferences.scan_labels.clone(),
    };
    let repos = scan_local(&body.path, &labels)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}

pub async fn scan_remote_handler(
    State(state): State<AppState>,
    body: Option<Json<RemoteScanRequest>>,
) -> Result<Json<Vec<DiscoveredRepo>>, (StatusCode, String)> {
    let token = state.auth.token().await;
    let client = GitHubClient::new(token).map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let labels = match body.and_then(|b| b.0.labels).filter(|l| !l.is_empty()) {
        Some(l) => l,
        None => state.config.read().await.preferences.scan_labels.clone(),
    };
    let repos = scan_remote(&client, &labels)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}

#[derive(Deserialize)]
pub struct LocalStreamRequest {
    pub path: PathBuf,
}

pub async fn scan_local_stream(
    State(state): State<AppState>,
    Json(body): Json<LocalStreamRequest>,
) -> Sse<impl futures::stream::Stream<Item = Result<Event, Infallible>>> {
    let labels = state.config.read().await.preferences.scan_labels.clone();
    let config = state.config.read().await.clone();
    let cancel = CancellationToken::new();
    let _scan_state = state.scan_state.clone();

    let (tx, rx) = mpsc::channel::<crate::scanner::ScanProgressEvent>(100);

    tokio::spawn(async move {
        let results = scan_local_with_progress(&body.path, &labels, cancel, |event| {
            let _ = tx.try_send(event);
        })
        .await
        .unwrap_or_default();

        // Persist local results
        let scan_results = ScanResults {
            last_scan_at: chrono::Utc::now(),
            local_results: results.clone(),
            remote_results: vec![],
            merged_results: results,
        };
        let _ = persistence::save_scan_results(&config.scan_results_path(), &scan_results).await;
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx).map(|event| {
        let json = serde_json::to_string(&event).unwrap_or_default();
        Ok(Event::default().data(json))
    });

    Sse::new(stream)
}

#[derive(Deserialize)]
pub struct CancelRequest {
    pub scan_id: String,
}

pub async fn cancel_scan(
    State(state): State<AppState>,
    Json(body): Json<CancelRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let cancelled = state.scan_state.cancel(&body.scan_id).await;
    Ok(Json(serde_json::json!({ "cancelled": cancelled })))
}

pub async fn get_scan_results(
    State(state): State<AppState>,
) -> Result<Json<Option<ScanResults>>, (StatusCode, String)> {
    let path = state.config.read().await.scan_results_path();
    let results = persistence::load_scan_results(&path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(results))
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
    async fn test_scan_local_accepts_labels_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();
        let body = serde_json::json!({
            "path": path,
            "labels": ["gpu"]
        })
        .to_string();

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

    #[tokio::test]
    async fn test_get_scan_results_empty_returns_null() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/scan/results")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json.is_null());
    }
}
