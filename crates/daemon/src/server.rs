use std::sync::Arc;

use crate::api::{service as api_service, updates as api_updates};
use anyhow::Result;
use axum::{
    routing::{delete, get, post},
    Json, Router,
};
use tokio::net::UnixListener;

use crate::api;
use crate::auth::AuthManager;
use crate::config::Config;
use crate::metrics::MetricsCollector;
use crate::notifications::NotificationManager;
use crate::runner::RunnerManager;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub auth: AuthManager,
    pub runner_manager: RunnerManager,
    pub metrics: Arc<MetricsCollector>,
    pub notifications: Arc<NotificationManager>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let runner_manager = RunnerManager::new(config.clone());
        Self {
            config: Arc::new(config),
            auth: AuthManager::new(),
            runner_manager,
            metrics: Arc::new(MetricsCollector::new()),
            notifications: Arc::new(NotificationManager::new()),
        }
    }

    #[cfg(test)]
    pub fn new_test() -> Self {
        let config = Config::with_base_dir(tempfile::tempdir().unwrap().keep().join(".homerun"));
        config.ensure_dirs().unwrap();
        Self::new(config)
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/auth/token", post(api::auth::login_with_token))
        .route("/auth", delete(api::auth::logout))
        .route("/auth/status", get(api::auth::status))
        .route("/repos", get(api::repos::list_repos))
        .route(
            "/runners",
            get(api::runners::list_runners).post(api::runners::create_runner),
        )
        .route(
            "/runners/{id}",
            get(api::runners::get_runner)
                .patch(api::runners::update_runner)
                .delete(api::runners::delete_runner),
        )
        .route("/runners/{id}/start", post(api::runners::start_runner))
        .route("/runners/{id}/stop", post(api::runners::stop_runner))
        .route("/runners/{id}/restart", post(api::runners::restart_runner))
        .route("/runners/{id}/logs", get(api::logs::stream_logs))
        .route("/events", get(api::events::events_ws))
        .route("/metrics", get(api::metrics::get_metrics))
        .route("/scan/local", post(api::scanner::scan_local_handler))
        .route("/scan/remote", post(api::scanner::scan_remote_handler))
        .route("/service/install", post(api_service::install_service))
        .route("/service/uninstall", post(api_service::uninstall_service))
        .route("/service/status", get(api_service::service_status))
        .route("/updates/check", get(api_updates::check_updates))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

pub async fn serve(config: Config) -> Result<()> {
    let socket_path = config.socket_path();

    // Remove stale socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Create parent directories
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    tracing::info!("Listening on Unix socket: {}", socket_path.display());

    let state = AppState::new(config);
    let app = create_router(state);

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_endpoint_contains_status_ok() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["status"], "ok");
        assert!(json.get("version").is_some());
    }

    #[tokio::test]
    async fn test_auth_status_unauthenticated() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/auth/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["authenticated"], false);
        assert!(json["user"].is_null());
    }

    #[tokio::test]
    async fn test_unknown_route_returns_404() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/nonexistent-route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_app_state_new_test_creates_valid_state() {
        let state = AppState::new_test();
        // Verify the state has sensible defaults
        assert!(!state.auth.status().await.authenticated);
        let runners = state.runner_manager.list().await;
        assert!(runners.is_empty());
    }
}
