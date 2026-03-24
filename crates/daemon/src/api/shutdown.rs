use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;

use crate::server::AppState;

pub async fn shutdown_daemon(
    State(state): State<AppState>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    if crate::launchd::is_daemon_installed() {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "Daemon is managed by launchd. Uninstall the service first or use `launchctl unload`."
            })),
        ));
    }

    tracing::info!("Shutdown requested via API");

    tokio::spawn(async move {
        let runners = state.runner_manager.list().await;
        for runner in &runners {
            if runner.state == crate::runner::state::RunnerState::Online
                || runner.state == crate::runner::state::RunnerState::Busy
            {
                tracing::info!("Stopping runner {} for shutdown", runner.config.name);
                if let Err(e) = state.runner_manager.stop_process(&runner.config.id).await {
                    tracing::warn!("Failed to stop runner {}: {}", runner.config.name, e);
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let socket_path = state.config.read().await.socket_path();
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
        tracing::info!("Daemon shutting down");
        std::process::exit(0);
    });

    Ok(StatusCode::ACCEPTED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::server::{create_router, AppState};

    #[tokio::test]
    async fn test_shutdown_returns_accepted() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/daemon/shutdown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // When launchd is NOT installed, we expect ACCEPTED.
        // When launchd IS installed, we expect CONFLICT.
        // In test environments the plist is unlikely to exist, so we expect ACCEPTED.
        if !crate::launchd::is_daemon_installed() {
            assert_eq!(response.status(), StatusCode::ACCEPTED);
        } else {
            assert_eq!(response.status(), StatusCode::CONFLICT);
        }
    }

    #[tokio::test]
    async fn test_shutdown_blocked_when_launchd_installed() {
        // This test verifies that when launchd IS installed, shutdown returns CONFLICT.
        // Since we can't easily mock is_daemon_installed(), we test the actual state.
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/daemon/shutdown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        if crate::launchd::is_daemon_installed() {
            assert_eq!(response.status(), StatusCode::CONFLICT);
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert!(json["error"].as_str().unwrap().contains("launchd"));
        } else {
            // If launchd is not installed, shutdown should be allowed
            assert_eq!(response.status(), StatusCode::ACCEPTED);
        }
    }
}
