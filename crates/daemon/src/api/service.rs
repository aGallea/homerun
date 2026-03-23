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

    let config = state.config.read().await;
    tracing::info!(
        config = ?*config,
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

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_service_status_returns_ok() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/service/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_service_status_returns_installed_bool() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/service/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json.get("installed").is_some());
        assert!(json["installed"].is_boolean());
    }

    #[tokio::test]
    async fn test_install_service_returns_ok_or_500() {
        // install_service calls launchctl which may not be usable in tests.
        // It should return either OK (installed) or INTERNAL_SERVER_ERROR (launchctl failed).
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/service/install")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        assert!(
            status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected status: {status}"
        );
    }

    #[tokio::test]
    async fn test_uninstall_service_returns_ok_or_500() {
        // uninstall_service should succeed (no-op) or error; never panic
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/service/uninstall")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        assert!(
            status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected status: {status}"
        );
    }
}
