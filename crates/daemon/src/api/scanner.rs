use axum::{
    extract::State,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
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
use uuid::Uuid;

use crate::github::GitHubClient;
use crate::scanner::persistence::{self, ScanResults};
use crate::scanner::{
    merge_results, scan_local, scan_local_with_progress, scan_remote, scan_remote_with_progress,
    DiscoveredRepo, ScanOutcome, ScanProgressEvent,
};
use crate::server::AppState;

#[derive(Clone, Default)]
pub struct ScanState {
    active_scans: Arc<Mutex<HashMap<String, CancellationToken>>>,
    persistence_lock: Arc<Mutex<()>>,
}

impl ScanState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, scan_id: String, cancel: CancellationToken) {
        self.active_scans.lock().await.insert(scan_id, cancel);
    }

    pub async fn cancel(&self, scan_id: &str) -> bool {
        if let Some(token) = self.active_scans.lock().await.get(scan_id).cloned() {
            token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn remove(&self, scan_id: &str) {
        self.active_scans.lock().await.remove(scan_id);
    }

    #[cfg(test)]
    async fn contains(&self, scan_id: &str) -> bool {
        self.active_scans.lock().await.contains_key(scan_id)
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
    pub scan_id: Option<String>,
}

#[derive(Default, Deserialize)]
pub struct StreamRequest {
    pub scan_id: Option<String>,
}

fn stream_event(event: ScanProgressEvent) -> Result<Event, Infallible> {
    Ok(Event::default()
        .data(serde_json::to_string(&event).expect("scan progress events must serialize")))
}

async fn persist_scan_outcome(
    state: &ScanState,
    path: &std::path::Path,
    scan_type: &str,
    outcome: &ScanOutcome,
) -> anyhow::Result<()> {
    let _guard = state.persistence_lock.lock().await;
    let existing = persistence::load_scan_results(path)
        .await?
        .unwrap_or(ScanResults {
            last_scan_at: chrono::Utc::now(),
            local_results: Vec::new(),
            remote_results: Vec::new(),
            merged_results: Vec::new(),
        });

    let (local_results, remote_results) = if scan_type == "local" {
        (outcome.results.clone(), existing.remote_results)
    } else {
        (existing.local_results, outcome.results.clone())
    };
    let merged_results = merge_results(local_results.clone(), remote_results.clone());
    persistence::save_scan_results(
        path,
        &ScanResults {
            last_scan_at: chrono::Utc::now(),
            local_results,
            remote_results,
            merged_results,
        },
    )
    .await
}

pub async fn scan_local_stream(
    State(state): State<AppState>,
    Json(body): Json<LocalStreamRequest>,
) -> Sse<impl futures::stream::Stream<Item = Result<Event, Infallible>>> {
    let labels = state.config.read().await.preferences.scan_labels.clone();
    let results_path = state.config.read().await.scan_results_path();
    let scan_id = body
        .scan_id
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let cancel = CancellationToken::new();
    state
        .scan_state
        .register(scan_id.clone(), cancel.clone())
        .await;

    let scan_state = state.scan_state.clone();
    let (tx, rx) = mpsc::unbounded_channel::<ScanProgressEvent>();
    let progress_tx = tx.clone();
    let task_scan_id = scan_id.clone();

    tokio::spawn(async move {
        let result =
            scan_local_with_progress(&body.path, &labels, &task_scan_id, cancel, move |event| {
                let _ = progress_tx.send(event);
            })
            .await;

        match result {
            Ok(outcome) if outcome.cancelled => {
                let _ = tx.send(ScanProgressEvent::Cancelled {
                    scan_id: task_scan_id.clone(),
                    scan_type: "local".to_string(),
                    checked: outcome.checked,
                    total: outcome.total,
                });
            }
            Ok(outcome) => {
                match persist_scan_outcome(&scan_state, &results_path, "local", &outcome).await {
                    Ok(()) => {
                        let _ = tx.send(ScanProgressEvent::Done {
                            scan_id: task_scan_id.clone(),
                            scan_type: "local".to_string(),
                            total_found: outcome.results.len(),
                            total_checked: outcome.checked,
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(ScanProgressEvent::Failed {
                            scan_id: task_scan_id.clone(),
                            scan_type: "local".to_string(),
                            message: format!("Failed to persist local scan results: {error:#}"),
                        });
                    }
                }
            }
            Err(error) => {
                let _ = tx.send(ScanProgressEvent::Failed {
                    scan_id: task_scan_id.clone(),
                    scan_type: "local".to_string(),
                    message: error.to_string(),
                });
            }
        }
        scan_state.remove(&task_scan_id).await;
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx).map(stream_event);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub async fn scan_remote_stream(
    State(state): State<AppState>,
    body: Option<Json<StreamRequest>>,
) -> Result<Sse<impl futures::stream::Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)>
{
    let token = state.auth.token().await;
    let client = GitHubClient::new(token).map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let labels = state.config.read().await.preferences.scan_labels.clone();
    let results_path = state.config.read().await.scan_results_path();
    let scan_id = body
        .and_then(|body| body.0.scan_id)
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let cancel = CancellationToken::new();
    state
        .scan_state
        .register(scan_id.clone(), cancel.clone())
        .await;

    let scan_state = state.scan_state.clone();
    let (tx, rx) = mpsc::unbounded_channel::<ScanProgressEvent>();
    let progress_tx = tx.clone();
    let task_scan_id = scan_id.clone();

    tokio::spawn(async move {
        let result =
            scan_remote_with_progress(&client, &labels, &task_scan_id, cancel, move |event| {
                let _ = progress_tx.send(event);
            })
            .await;

        match result {
            Ok(outcome) if outcome.cancelled => {
                let _ = tx.send(ScanProgressEvent::Cancelled {
                    scan_id: task_scan_id.clone(),
                    scan_type: "remote".to_string(),
                    checked: outcome.checked,
                    total: outcome.total,
                });
            }
            Ok(outcome) => {
                match persist_scan_outcome(&scan_state, &results_path, "remote", &outcome).await {
                    Ok(()) => {
                        let _ = tx.send(ScanProgressEvent::Done {
                            scan_id: task_scan_id.clone(),
                            scan_type: "remote".to_string(),
                            total_found: outcome.results.len(),
                            total_checked: outcome.checked,
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(ScanProgressEvent::Failed {
                            scan_id: task_scan_id.clone(),
                            scan_type: "remote".to_string(),
                            message: format!("Failed to persist remote scan results: {error:#}"),
                        });
                    }
                }
            }
            Err(error) => {
                let _ = tx.send(ScanProgressEvent::Failed {
                    scan_id: task_scan_id.clone(),
                    scan_type: "remote".to_string(),
                    message: error.to_string(),
                });
            }
        }
        scan_state.remove(&task_scan_id).await;
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx).map(stream_event);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
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
    async fn test_scan_state_registers_and_cancels_by_id() {
        let state = super::ScanState::new();
        let token = tokio_util::sync::CancellationToken::new();
        state.register("scan-1".to_string(), token.clone()).await;

        assert!(state.contains("scan-1").await);
        assert!(state.cancel("scan-1").await);
        assert!(token.is_cancelled());
        state.remove("scan-1").await;
        assert!(!state.contains("scan-1").await);
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
