use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::runner::types::{CreateRunnerRequest, RunnerInfo, UpdateRunnerRequest};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct ListRunnersQuery {
    pub group_id: Option<String>,
}

pub async fn create_runner(
    State(state): State<AppState>,
    Json(req): Json<CreateRunnerRequest>,
) -> Result<(StatusCode, Json<RunnerInfo>), (StatusCode, String)> {
    crate::runner::RunnerManager::validate_create_request(
        &req.repo_full_name,
        req.name.as_deref(),
        req.mode.as_ref(),
        req.container.as_ref(),
    )
    .map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;

    // Authenticate before persisting a Creating runner. Otherwise a 401 leaves
    // an orphaned local runner that never had a chance to start.
    let token = state.auth.token().await.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "No auth token available. Please authenticate first.".to_string(),
        )
    })?;

    let runner = state
        .runner_manager
        .create_desired_running(
            &req.repo_full_name,
            req.name,
            req.labels,
            req.mode,
            None,
            req.container,
        )
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Desired-running creation atomically persists intent and reserves the
    // start under the daemon lifecycle barrier before returning here.
    let runner_id = runner.config.id.clone();
    let manager = state.runner_manager.clone();
    tokio::spawn(async move {
        if let Err(e) = manager.start_existing_reserved(&runner_id, &token).await {
            tracing::error!("Failed to register and start runner {}: {}", runner_id, e);
            let _ = manager
                .update_state_with_error(
                    &runner_id,
                    crate::runner::state::RunnerState::Error,
                    Some(format!("{e:#}")),
                )
                .await;
            manager.schedule_recovery(runner_id.clone());
        }
        manager.finish_start_operation(&runner_id).await;
    });

    Ok((StatusCode::CREATED, Json(runner)))
}

pub async fn list_runners(
    State(state): State<AppState>,
    Query(query): Query<ListRunnersQuery>,
) -> Json<Vec<RunnerInfo>> {
    match query.group_id {
        Some(gid) => Json(state.runner_manager.list_by_group(&gid).await),
        None => Json(state.runner_manager.list().await),
    }
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
    if state.runner_manager.get(&id).await.is_none() {
        return Err((StatusCode::NOT_FOUND, format!("Runner '{id}' not found")));
    }

    // Validate before the manager mutates labels/mode, so an invalid alias makes
    // the entire PATCH fail without partial changes.
    if let Some(value) = req.display_name.clone() {
        crate::runner::types::normalize_display_name(value)
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    }

    let updated = state.runner_manager.update(&id, req).await.map_err(|e| {
        let message = e.to_string();
        if message == "Runner not found" {
            (StatusCode::NOT_FOUND, message)
        } else if message.contains("cannot be changed")
            || message.contains("only be changed while the runner is stopped")
            || message.contains("operation in progress")
            || message.contains("being deleted")
            || message.contains("being updated")
        {
            (StatusCode::CONFLICT, message)
        } else if message.contains("persisting runner update") {
            (StatusCode::INTERNAL_SERVER_ERROR, message)
        } else {
            (StatusCode::BAD_REQUEST, message)
        }
    })?;

    Ok(Json(updated))
}

pub async fn delete_runner(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .runner_manager
        .get(&id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))?;

    let token = state.auth.token().await;
    if let Some(token) = token {
        state
            .runner_manager
            .full_delete(&id, &token)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to delete runner: {error}"),
                )
            })?;
    } else {
        state.runner_manager.delete(&id).await.map_err(|error| {
            let message = error.to_string();
            if message.contains("Authentication required") {
                (StatusCode::UNAUTHORIZED, message)
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, message)
            }
        })?;
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
    if state.runner_manager.has_active_process(&id).await {
        return Err((
            StatusCode::CONFLICT,
            "Runner still has an active process and must be stopped before starting again"
                .to_string(),
        ));
    }

    let token = state.auth.token().await.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "No auth token available. Please authenticate first.".to_string(),
        )
    })?;

    state
        .runner_manager
        .begin_start_operation(&id)
        .await
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;
    if let Err(error) = state.runner_manager.set_desired_running(&id, true).await {
        state.runner_manager.finish_start_operation(&id).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to persist start intent: {error}"),
        ));
    }

    let manager = state.runner_manager.clone();
    let runner_id = id.clone();
    tokio::spawn(async move {
        if let Err(e) = manager.start_existing_reserved(&runner_id, &token).await {
            tracing::error!("Failed to start runner {}: {}", runner_id, e);
            let _ = manager
                .update_state_with_error(
                    &runner_id,
                    crate::runner::state::RunnerState::Error,
                    Some(format!("{e:#}")),
                )
                .await;
            manager.schedule_recovery(runner_id.clone());
        }
        manager.finish_start_operation(&runner_id).await;
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
        && !state.runner_manager.has_active_process(&id).await
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

    if !matches!(
        runner.state,
        crate::runner::state::RunnerState::Online
            | crate::runner::state::RunnerState::Busy
            | crate::runner::state::RunnerState::Offline
            | crate::runner::state::RunnerState::Error
    ) {
        return Err((
            StatusCode::CONFLICT,
            format!("Runner is in {:?} state, cannot restart", runner.state),
        ));
    }
    if matches!(
        runner.state,
        crate::runner::state::RunnerState::Offline | crate::runner::state::RunnerState::Error
    ) && state.runner_manager.has_active_process(&id).await
    {
        return Err((
            StatusCode::CONFLICT,
            "Runner still has an active process and must be stopped before restarting".to_string(),
        ));
    }

    // Authenticate before stopping. A failed restart must not take a healthy
    // runner offline merely because credentials are unavailable.
    let token = state.auth.token().await.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "No auth token available. Please authenticate first.".to_string(),
        )
    })?;

    // Reserve the full stop -> start sequence before mutating state. This
    // makes concurrent Start/Restart/Delete/PATCH requests deterministic.
    state
        .runner_manager
        .begin_start_operation(&id)
        .await
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;

    if let Err(error) = state.runner_manager.set_desired_running(&id, true).await {
        state.runner_manager.finish_start_operation(&id).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to persist restart intent: {error}"),
        ));
    }

    // Stop if running, retaining the already-persisted restart intent.
    if runner.state == crate::runner::state::RunnerState::Online
        || runner.state == crate::runner::state::RunnerState::Busy
        || state.runner_manager.has_active_process(&id).await
    {
        if let Err(error) = state.runner_manager.stop_process_internal(&id, false).await {
            state.runner_manager.finish_start_operation(&id).await;
            state.runner_manager.schedule_recovery(id.clone());
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to stop runner for restart: {error}"),
            ));
        }
    }

    // Now start — the old process is fully stopped and deletion remains blocked.
    let manager = state.runner_manager.clone();
    let runner_id = id.clone();
    tokio::spawn(async move {
        if let Err(e) = manager.start_existing_reserved(&runner_id, &token).await {
            tracing::error!("Failed to restart runner {}: {}", runner_id, e);
            let _ = manager
                .update_state_with_error(
                    &runner_id,
                    crate::runner::state::RunnerState::Error,
                    Some(format!("{e:#}")),
                )
                .await;
            manager.schedule_recovery(runner_id.clone());
        }
        manager.finish_start_operation(&runner_id).await;
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
    async fn test_start_reserves_before_publishing_registering_state() {
        let state = AppState::new_test_authenticated();
        let runner = state
            .runner_manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;
        state
            .runner_manager
            .update_state(&id, crate::runner::state::RunnerState::Error)
            .await
            .unwrap();
        state
            .runner_manager
            .begin_stop_operation(&id)
            .await
            .unwrap();

        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/runners/{id}/start"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            state.runner_manager.get(&id).await.unwrap().state,
            crate::runner::state::RunnerState::Error
        );
        state.runner_manager.finish_stop_operation(&id).await;
    }

    #[tokio::test]
    async fn test_restart_rejects_transitional_state_before_authentication() {
        let state = AppState::new_test();
        let runner = state
            .runner_manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/runners/{}/restart", runner.config.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_create_and_list_runners() {
        let state = AppState::new_test_authenticated();

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
        let state = AppState::new_test_authenticated();

        // Seed an unconfigured runner directly. Creating through POST /runners
        // intentionally starts registration in the background, which makes this
        // DELETE endpoint test depend on runner downloads and GitHub network timing.
        let runner = state
            .runner_manager
            .create("aGallea/gifted", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

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
    async fn test_delete_configured_runner_requires_authentication() {
        let state = AppState::new_test();
        let runner = state
            .runner_manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();
        std::fs::write(runner.config.work_dir.join(".runner"), "configured").unwrap();

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

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(state.runner_manager.get(&id).await.is_some());
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
        let state = AppState::new_test_authenticated();
        let runner = state
            .runner_manager
            .create("aGallea/gifted", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        // Update labels on a stopped fixture. POST /runners intentionally starts
        // asynchronously and is covered by dedicated create/start tests.
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
    async fn test_update_runner_rejects_invalid_display_name_without_partial_changes() {
        let state = AppState::new_test();
        let runner = state
            .runner_manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();
        let original_labels = runner.config.labels.clone();

        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/runners/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"labels":["changed"],"mode":"service","display_name":"bad\nname"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let unchanged = state.runner_manager.get(&id).await.unwrap();
        assert_eq!(unchanged.config.labels, original_labels);
        assert_eq!(unchanged.config.mode, crate::runner::types::RunnerMode::App);
        assert_eq!(unchanged.config.display_name, None);
    }

    #[tokio::test]
    async fn test_update_runner_trims_and_clears_display_name() {
        let state = AppState::new_test();
        let runner = state
            .runner_manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id.clone();

        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/runners/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"display_name":"  Office runner  "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            state
                .runner_manager
                .get(&id)
                .await
                .unwrap()
                .config
                .display_name
                .as_deref(),
            Some("Office runner")
        );

        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/runners/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"display_name":null}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            state
                .runner_manager
                .get(&id)
                .await
                .unwrap()
                .config
                .display_name,
            None
        );
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

    /// Helper: create a runner and return its ID string.
    async fn create_runner_and_get_id(state: &crate::server::AppState) -> String {
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        runner["config"]["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_stop_runner_conflict_when_not_online() {
        // A newly created runner is in "creating" state → stop should return 409 CONFLICT
        let state = AppState::new_test_authenticated();
        let id = create_runner_and_get_id(&state).await;

        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/runners/{id}/stop"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_start_runner_conflict_when_not_offline_or_error() {
        // A newly created runner is in "creating" state → start should return 409 CONFLICT
        // because start only accepts Offline or Error states.
        let state = AppState::new_test_authenticated();
        let id = create_runner_and_get_id(&state).await;

        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/runners/{id}/start"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_restart_runner_in_offline_state_spawns_ok() {
        // A runner that is Offline can be restarted (200 OK) – the actual registration
        // is async and we don't wait for it.  We just verify the HTTP response.
        use crate::runner::state::RunnerState;

        let state = AppState::new_test_authenticated();
        let runner = state
            .runner_manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        // Manually transition a stopped fixture to Offline so restart is valid.
        state
            .runner_manager
            .update_state(&id, RunnerState::Registering)
            .await
            .unwrap();
        state
            .runner_manager
            .update_state(&id, RunnerState::Online)
            .await
            .unwrap();
        state
            .runner_manager
            .update_state(&id, RunnerState::Offline)
            .await
            .unwrap();

        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/runners/{id}/restart"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Offline runner can be restarted (it goes through start path, not stop)
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_delete_runner_not_found() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/runners/nonexistent-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_create_runner_with_name_and_labels() {
        let state = AppState::new_test_authenticated();
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"repo_full_name":"owner/myrepo","name":"custom-name","labels":["gpu"]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(runner["config"]["name"], "custom-name");
        let labels = runner["config"]["labels"].as_array().unwrap();
        assert!(labels.iter().any(|l| l.as_str() == Some("gpu")));
        // User-provided labels are used as-is — no defaults merged
        assert_eq!(labels.len(), 1);
    }

    #[tokio::test]
    async fn test_create_runner_invalid_repo_name() {
        // Repo name without '/' is invalid
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"nodash"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_update_runner_not_found() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/runners/nonexistent-id")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"labels":["self-hosted"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_runner_native_mode_while_stopped() {
        let state = AppState::new_test_authenticated();
        let runner = state
            .runner_manager
            .create("owner/repo", None, None, None, None, None)
            .await
            .unwrap();
        let id = runner.config.id;

        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/runners/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"mode":"service"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(updated["config"]["mode"], "service");
    }

    #[tokio::test]
    async fn test_get_runner_returns_correct_id() {
        let state = AppState::new_test_authenticated();
        let id = create_runner_and_get_id(&state).await;

        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/runners/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(runner["config"]["id"].as_str().unwrap(), id);
    }
}
