use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::runner::types::{CreateRunnerRequest, RunnerInfo, UpdateRunnerRequest};
use crate::server::AppState;

pub async fn create_runner(
    State(state): State<AppState>,
    Json(req): Json<CreateRunnerRequest>,
) -> Result<(StatusCode, Json<RunnerInfo>), (StatusCode, String)> {
    let runner = state
        .runner_manager
        .create(&req.repo_full_name, req.name, req.labels, req.mode)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Spawn background task to register and start the runner
    let manager = state.runner_manager.clone();
    let auth = state.auth.clone();
    let runner_id = runner.config.id.clone();
    tokio::spawn(async move {
        let token = match auth.token().await {
            Some(t) => t,
            None => {
                tracing::error!("No auth token available for runner registration");
                let _ = manager
                    .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                    .await;
                return;
            }
        };
        if let Err(e) = manager.register_and_start(&runner_id, &token).await {
            tracing::error!("Failed to register and start runner {}: {}", runner_id, e);
            let _ = manager
                .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                .await;
        }
    });

    Ok((StatusCode::CREATED, Json(runner)))
}

pub async fn list_runners(State(state): State<AppState>) -> Json<Vec<RunnerInfo>> {
    Json(state.runner_manager.list().await)
}

pub async fn get_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RunnerInfo>, (StatusCode, String)> {
    state
        .runner_manager
        .get(&id)
        .await
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))
}

pub async fn update_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRunnerRequest>,
) -> Result<Json<RunnerInfo>, (StatusCode, String)> {
    state
        .runner_manager
        .update(&id, req)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))
}

pub async fn delete_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let runner = state
        .runner_manager
        .get(&id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))?;

    let token = state.auth.token().await;
    if let Some(token) = token {
        // Full delete with deregistration
        if runner.state == crate::runner::state::RunnerState::Online
            || runner.state == crate::runner::state::RunnerState::Busy
            || runner.state == crate::runner::state::RunnerState::Offline
        {
            state
                .runner_manager
                .full_delete(&id, &token)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to delete runner: {e}"),
                    )
                })?;
        } else {
            state
                .runner_manager
                .delete(&id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    } else {
        // No auth token, just remove locally
        state
            .runner_manager
            .delete(&id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn start_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let runner = state
        .runner_manager
        .get(&id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))?;

    if runner.state != crate::runner::state::RunnerState::Offline
        && runner.state != crate::runner::state::RunnerState::Error
    {
        return Err((
            StatusCode::CONFLICT,
            format!("Runner is in {:?} state, cannot start", runner.state),
        ));
    }

    let manager = state.runner_manager.clone();
    let auth = state.auth.clone();
    let runner_id = id.clone();
    tokio::spawn(async move {
        let token = match auth.token().await {
            Some(t) => t,
            None => {
                tracing::error!("No auth token available for runner start");
                return;
            }
        };
        // Offline/Error -> Registering is a valid transition
        if let Err(e) = manager
            .update_state(&runner_id, crate::runner::state::RunnerState::Registering)
            .await
        {
            tracing::error!("Failed to transition runner {}: {}", runner_id, e);
            return;
        }
        if let Err(e) = manager
            .register_and_start_from_registering(&runner_id, &token)
            .await
        {
            tracing::error!("Failed to start runner {}: {}", runner_id, e);
            let _ = manager
                .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                .await;
        }
    });

    Ok(StatusCode::OK)
}

pub async fn stop_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let runner = state
        .runner_manager
        .get(&id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))?;

    if runner.state != crate::runner::state::RunnerState::Online
        && runner.state != crate::runner::state::RunnerState::Busy
    {
        return Err((
            StatusCode::CONFLICT,
            format!("Runner is in {:?} state, cannot stop", runner.state),
        ));
    }

    state.runner_manager.stop_process(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to stop runner: {e}"),
        )
    })?;

    Ok(StatusCode::OK)
}

pub async fn restart_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let runner = state
        .runner_manager
        .get(&id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))?;

    // Stop if running
    if runner.state == crate::runner::state::RunnerState::Online
        || runner.state == crate::runner::state::RunnerState::Busy
    {
        state.runner_manager.stop_process(&id).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to stop runner: {e}"),
            )
        })?;
    }

    // Now start
    let manager = state.runner_manager.clone();
    let auth = state.auth.clone();
    let runner_id = id.clone();
    tokio::spawn(async move {
        let token = match auth.token().await {
            Some(t) => t,
            None => {
                tracing::error!("No auth token available for runner restart");
                return;
            }
        };
        if let Err(e) = manager
            .update_state(&runner_id, crate::runner::state::RunnerState::Registering)
            .await
        {
            tracing::error!("Failed to transition runner {}: {}", runner_id, e);
            return;
        }
        if let Err(e) = manager
            .register_and_start_from_registering(&runner_id, &token)
            .await
        {
            tracing::error!("Failed to restart runner {}: {}", runner_id, e);
            let _ = manager
                .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                .await;
        }
    });

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_create_and_list_runners() {
        let state = AppState::new_test();

        // Create a runner
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"aGallea/gifted"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        // List runners (recreate router -- Router is consumed by oneshot)
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(runners.len(), 1);
    }

    #[tokio::test]
    async fn test_delete_runner() {
        let state = AppState::new_test();

        // Create
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"aGallea/gifted"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id = runner["config"]["id"].as_str().unwrap();

        // Delete
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/runners/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // Verify gone
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(runners.len(), 0);
    }

    #[tokio::test]
    async fn test_get_runner_not_found() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/nonexistent-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_runner() {
        let state = AppState::new_test();

        // Create
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"aGallea/gifted"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id = runner["config"]["id"].as_str().unwrap();

        // Update labels
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/runners/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"labels":["self-hosted","custom-label"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let labels = updated["config"]["labels"].as_array().unwrap();
        assert!(labels.iter().any(|l| l.as_str() == Some("custom-label")));
    }

    #[tokio::test]
    async fn test_start_stop_restart_runner_not_found() {
        let state = AppState::new_test();

        for action in &["start", "stop", "restart"] {
            let app = create_router(state.clone());
            let response = app
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/runners/nonexistent-id/{action}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "action={action} should return NOT_FOUND for nonexistent runner"
            );
        }
    }
}
