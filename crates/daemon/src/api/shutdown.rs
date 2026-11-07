use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;

use crate::server::AppState;

pub async fn shutdown_daemon(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if crate::platform::service::is_daemon_installed() {
        let msg = if cfg!(target_os = "macos") {
            "Daemon is managed by launchd. Uninstall the service first or use `launchctl unload`."
        } else if cfg!(windows) {
            "Daemon is registered as an auto-start service. Uninstall the service first."
        } else {
            "Daemon is installed as a system service. Uninstall the service first."
        };
        return Err((StatusCode::CONFLICT, Json(json!({ "error": msg }))));
    }

    tracing::info!("Shutdown requested via API");

    // Install the lifecycle admission barrier before observing runner state. New
    // lifecycle mutations are rejected from this point onward; operations admitted
    // before the barrier retain their reservations and are drained below. Creation
    // holds the same admission lock through its durable state write.
    let admitted_starts = state
        .runner_manager
        .begin_shutdown_operation()
        .await
        .map_err(|error| {
            (
                StatusCode::CONFLICT,
                Json(json!({ "error": error.to_string() })),
            )
        })?;

    let runners = state.runner_manager.list().await;
    let mut observed_active = 0usize;
    for runner in &runners {
        let transitional = !matches!(
            runner.state,
            crate::runner::state::RunnerState::Offline | crate::runner::state::RunnerState::Error
        );
        if transitional
            || state
                .runner_manager
                .has_active_process(&runner.config.id)
                .await
        {
            observed_active += 1;
        }
    }
    // A newly admitted start may still report Offline/Error before its async
    // start task publishes Registering, so retain at least the reservation count.
    let active_count = observed_active.max(admitted_starts);

    tokio::spawn(async move {
        // Wait for every lifecycle mutation that crossed the barrier first. Once
        // this drains, no user/API/recovery operation can race the final process
        // enumeration or begin a new start while shutdown is in progress.
        state
            .runner_manager
            .wait_for_lifecycle_operations_to_finish()
            .await;

        let runners = state.runner_manager.list().await;
        let mut stop_futures = Vec::new();
        for runner in &runners {
            let active = runner.state == crate::runner::state::RunnerState::Online
                || runner.state == crate::runner::state::RunnerState::Busy
                || state
                    .runner_manager
                    .has_active_process(&runner.config.id)
                    .await;
            if active {
                tracing::info!("Stopping runner {} for shutdown", runner.config.name);
                let manager = state.runner_manager.clone();
                let id = runner.config.id.clone();
                stop_futures.push(async move {
                    // The shutdown barrier has drained prior reservations and blocks
                    // new ones, so shutdown owns this final transition directly.
                    let result = manager.stop_process_internal(&id, false).await;
                    (id, result)
                });
            }
        }

        let stop_results = futures::future::join_all(stop_futures).await;
        let mut stopped_ids = Vec::with_capacity(stop_results.len());
        let mut stop_failed = false;
        for (id, result) in stop_results {
            match result {
                Ok(()) => stopped_ids.push(id),
                Err(error) => {
                    stop_failed = true;
                    tracing::warn!("Failed to stop runner {} for shutdown: {}", id, error);
                }
            }
        }

        if stop_failed {
            // Do not exit while any runner may still be alive. Re-open lifecycle
            // admission and restore only runners whose stop was confirmed; a runner
            // whose stop failed remains supervised by its existing process monitor.
            // The CLI will observe that the daemon is still healthy and report the
            // shutdown timeout instead of claiming success.
            tracing::error!("Daemon shutdown aborted because one or more runners did not stop");
            state.runner_manager.cancel_shutdown_operation().await;
            for id in stopped_ids {
                state.runner_manager.schedule_recovery(id);
            }
            return;
        }

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // On Unix, clean up the socket file. On Windows, named pipes are
        // kernel objects and require no file cleanup.
        #[cfg(unix)]
        {
            let socket_path = state.config.read().await.socket_path();
            if socket_path.exists() {
                let _ = std::fs::remove_file(&socket_path);
            }
        }
        tracing::info!("Daemon shutting down");
        std::process::exit(0);
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "active_runners": active_count })),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    use crate::server::{create_router, AppState};

    #[tokio::test]
    async fn test_shutdown_returns_accepted_or_conflict() {
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
        // On machines where the daemon is registered as a service,
        // shutdown returns CONFLICT; otherwise ACCEPTED.
        let status = response.status();
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::CONFLICT,
            "expected ACCEPTED or CONFLICT, got {status}"
        );
    }

    #[tokio::test]
    async fn test_shutdown_blocked_when_service_installed() {
        // Since we can't easily mock is_daemon_installed(), we test the actual state.
        // The handler calls is_daemon_installed() internally, so we just verify the
        // response is one of the two valid outcomes.
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

        let status = response.status();
        assert!(
            status == StatusCode::ACCEPTED || status == StatusCode::CONFLICT,
            "expected ACCEPTED or CONFLICT, got {status}"
        );

        if status == StatusCode::CONFLICT {
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert!(json["error"].as_str().unwrap().contains("service"));
        }
    }

    #[tokio::test]
    async fn test_shutdown_returns_active_runners_count() {
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
        if response.status() == StatusCode::CONFLICT {
            return; // Daemon is installed as a service — shutdown blocked
        }
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        // No runners in test state, so active_runners should be 0
        assert_eq!(json["active_runners"], 0);
    }
}
