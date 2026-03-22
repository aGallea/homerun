use axum::{extract::State, Json};

use crate::server::AppState;

pub async fn get_metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let system = state.metrics.system_snapshot();
    let runners = state.runner_manager.list().await;
    // Refresh process list once so all runners read from the same snapshot
    state.metrics.refresh_processes();
    let runner_metrics: Vec<_> = runners
        .iter()
        .filter_map(|r| {
            r.pid.and_then(|pid| {
                state.metrics.runner_metrics(pid).map(|mut m| {
                    m.runner_id = r.config.id.clone();
                    m
                })
            })
        })
        .collect();
    let runner_pids = state.runner_manager.runner_pids_and_names().await;
    let uptime = state.daemon_start_time.elapsed();
    let daemon = state
        .metrics
        .daemon_metrics(state.daemon_pid, uptime, &runner_pids);

    Json(serde_json::json!({ "system": system, "runners": runner_metrics, "daemon": daemon }))
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_metrics_returns_ok() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_metrics_has_system_and_runners_keys() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json.get("system").is_some(),
            "response should have 'system' key"
        );
        assert!(
            json.get("runners").is_some(),
            "response should have 'runners' key"
        );
        assert!(json["runners"].is_array());
    }

    #[tokio::test]
    async fn test_get_metrics_system_fields() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let system = &json["system"];
        assert!(system.get("cpu_percent").is_some());
        assert!(system.get("memory_used_bytes").is_some());
        assert!(system.get("memory_total_bytes").is_some());
        assert!(system.get("disk_used_bytes").is_some());
        assert!(system.get("disk_total_bytes").is_some());
    }
}
