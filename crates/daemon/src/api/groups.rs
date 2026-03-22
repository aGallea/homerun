use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::runner::state::RunnerState;
use crate::runner::types::{
    BatchCreateResponse, CreateBatchRequest, GroupActionResponse, GroupActionResult,
    ScaleGroupRequest, ScaleGroupResponse,
};
use crate::server::AppState;

pub async fn create_batch(
    State(state): State<AppState>,
    Json(req): Json<CreateBatchRequest>,
) -> Result<(StatusCode, Json<BatchCreateResponse>), (StatusCode, String)> {
    if req.count < 2 || req.count > 10 {
        return Err((
            StatusCode::BAD_REQUEST,
            "count must be between 2 and 10".to_string(),
        ));
    }

    let (group_id, runners, errors) = state
        .runner_manager
        .create_batch(&req.repo_full_name, req.count, req.labels, req.mode)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    // Spawn background registration for each runner
    for runner in &runners {
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
                tracing::error!("Failed to register runner {}: {}", runner_id, e);
                let _ = manager
                    .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                    .await;
            }
        });
    }

    let status = if errors.is_empty() {
        StatusCode::CREATED
    } else {
        StatusCode::MULTI_STATUS
    };

    Ok((
        status,
        Json(BatchCreateResponse {
            group_id,
            runners,
            errors,
        }),
    ))
}

pub async fn start_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupActionResponse>, (StatusCode, String)> {
    let runners = state.runner_manager.list_by_group(&group_id).await;
    if runners.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("No runners found for group '{group_id}'"),
        ));
    }

    let mut results = Vec::new();
    for runner in &runners {
        let id = runner.config.id.clone();
        if runner.state == RunnerState::Offline || runner.state == RunnerState::Error {
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
                if let Err(e) = manager
                    .update_state(&runner_id, RunnerState::Registering)
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
                    let _ = manager.update_state(&runner_id, RunnerState::Error).await;
                }
            });
            results.push(GroupActionResult {
                runner_id: id,
                success: true,
                error: None,
            });
        } else {
            results.push(GroupActionResult {
                runner_id: id,
                success: false,
                error: Some(format!(
                    "Runner is in {:?} state, cannot start",
                    runner.state
                )),
            });
        }
    }

    Ok(Json(GroupActionResponse { group_id, results }))
}

pub async fn stop_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupActionResponse>, (StatusCode, String)> {
    let runners = state.runner_manager.list_by_group(&group_id).await;
    if runners.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("No runners found for group '{group_id}'"),
        ));
    }

    let mut results = Vec::new();
    for runner in &runners {
        let id = runner.config.id.clone();
        if runner.state == RunnerState::Online || runner.state == RunnerState::Busy {
            match state.runner_manager.stop_process(&id).await {
                Ok(_) => results.push(GroupActionResult {
                    runner_id: id,
                    success: true,
                    error: None,
                }),
                Err(e) => results.push(GroupActionResult {
                    runner_id: id,
                    success: false,
                    error: Some(format!("Failed to stop runner: {e}")),
                }),
            }
        } else {
            results.push(GroupActionResult {
                runner_id: id,
                success: false,
                error: Some(format!(
                    "Runner is in {:?} state, cannot stop",
                    runner.state
                )),
            });
        }
    }

    Ok(Json(GroupActionResponse { group_id, results }))
}

pub async fn restart_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupActionResponse>, (StatusCode, String)> {
    let runners = state.runner_manager.list_by_group(&group_id).await;
    if runners.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("No runners found for group '{group_id}'"),
        ));
    }

    let results: Vec<GroupActionResult> = runners
        .iter()
        .map(|r| GroupActionResult {
            runner_id: r.config.id.clone(),
            success: true,
            error: None,
        })
        .collect();

    // Spawn a single background task to stop all then restart all
    let manager = state.runner_manager.clone();
    let auth = state.auth.clone();
    let group_id_clone = group_id.clone();
    tokio::spawn(async move {
        let token = match auth.token().await {
            Some(t) => t,
            None => {
                tracing::error!("No auth token available for group restart");
                return;
            }
        };

        let runners = manager.list_by_group(&group_id_clone).await;

        // Stop all running runners concurrently
        let mut stop_handles = Vec::new();
        for runner in &runners {
            if runner.state == RunnerState::Online
                || runner.state == RunnerState::Busy
                || runner.state == RunnerState::Stopping
            {
                let mgr = manager.clone();
                let rid = runner.config.id.clone();
                stop_handles.push(tokio::spawn(async move {
                    let _ = mgr.stop_process(&rid).await;
                }));
            }
        }
        for handle in stop_handles {
            let _ = handle.await;
        }

        // Now restart all
        for runner in &runners {
            let mgr = manager.clone();
            let rid = runner.config.id.clone();
            let tok = token.clone();
            tokio::spawn(async move {
                if let Err(e) = mgr.update_state(&rid, RunnerState::Registering).await {
                    tracing::error!("Failed to transition runner {}: {}", rid, e);
                    return;
                }
                if let Err(e) = mgr.register_and_start_from_registering(&rid, &tok).await {
                    tracing::error!("Failed to restart runner {}: {}", rid, e);
                    let _ = mgr.update_state(&rid, RunnerState::Error).await;
                }
            });
        }
    });

    Ok(Json(GroupActionResponse { group_id, results }))
}

pub async fn scale_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
    Json(req): Json<ScaleGroupRequest>,
) -> Result<Json<ScaleGroupResponse>, (StatusCode, String)> {
    if req.count < 1 || req.count > 10 {
        return Err((
            StatusCode::BAD_REQUEST,
            "count must be between 1 and 10".to_string(),
        ));
    }

    let response = state
        .runner_manager
        .scale_group(&group_id, req.count)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    // Spawn registration for added runners
    for runner in &response.added {
        let manager = state.runner_manager.clone();
        let auth = state.auth.clone();
        let runner_id = runner.config.id.clone();
        tokio::spawn(async move {
            let token = match auth.token().await {
                Some(t) => t,
                None => {
                    let _ = manager
                        .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                        .await;
                    return;
                }
            };
            if let Err(e) = manager.register_and_start(&runner_id, &token).await {
                tracing::error!("Failed to register runner {}: {}", runner_id, e);
                let _ = manager
                    .update_state(&runner_id, crate::runner::state::RunnerState::Error)
                    .await;
            }
        });
    }

    Ok(Json(response))
}

pub async fn delete_group(
    State(state): State<AppState>,
    Path(group_id): Path<String>,
) -> Result<Json<GroupActionResponse>, (StatusCode, String)> {
    let runners = state.runner_manager.list_by_group(&group_id).await;
    if runners.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("No runners found for group '{group_id}'"),
        ));
    }

    let token = state.auth.token().await;
    let mut results = Vec::new();

    for runner in &runners {
        let id = runner.config.id.clone();

        if runner.state == RunnerState::Busy {
            results.push(GroupActionResult {
                runner_id: id,
                success: false,
                error: Some("Runner is Busy, cannot delete".to_string()),
            });
            continue;
        }

        let delete_result = if let Some(ref t) = token {
            if runner.state == RunnerState::Online
                || runner.state == RunnerState::Offline
                || runner.state == RunnerState::Error
            {
                state.runner_manager.full_delete(&id, t).await
            } else {
                state.runner_manager.delete(&id).await
            }
        } else {
            state.runner_manager.delete(&id).await
        };

        match delete_result {
            Ok(_) => results.push(GroupActionResult {
                runner_id: id,
                success: true,
                error: None,
            }),
            Err(e) => results.push(GroupActionResult {
                runner_id: id,
                success: false,
                error: Some(format!("Failed to delete runner: {e}")),
            }),
        }
    }

    Ok(Json(GroupActionResponse { group_id, results }))
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_batch_create_returns_group_id_and_runners() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/myrepo","count":3}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(resp["group_id"].is_string());
        assert_eq!(resp["runners"].as_array().unwrap().len(), 3);
        assert_eq!(resp["errors"].as_array().unwrap().len(), 0);

        let gid = resp["group_id"].as_str().unwrap();
        for runner in resp["runners"].as_array().unwrap() {
            assert_eq!(runner["config"]["group_id"].as_str().unwrap(), gid);
        }
    }

    #[tokio::test]
    async fn test_batch_create_auto_names_with_counter() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/myrepo","count":2}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let names: Vec<&str> = resp["runners"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["config"]["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, vec!["myrepo-runner-1", "myrepo-runner-2"]);
    }

    #[tokio::test]
    async fn test_batch_create_rejects_count_below_2() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":1}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_batch_create_rejects_count_above_10() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":11}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_group_start_returns_results() {
        let state = AppState::new_test();
        // Create a batch
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":2}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let group_id = batch["group_id"].as_str().unwrap();

        // Start the group (runners are in Creating state, can't start)
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/runners/groups/{group_id}/start"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp["group_id"].as_str().unwrap(), group_id);
        assert_eq!(resp["results"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_group_action_404_for_nonexistent_group() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/groups/nonexistent-group/start")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_scale_up_adds_runners() {
        let state = AppState::new_test();
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":2}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let group_id = batch["group_id"].as_str().unwrap();

        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/runners/groups/{group_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"count":4}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp["previous_count"].as_u64().unwrap(), 2);
        assert_eq!(resp["actual_count"].as_u64().unwrap(), 4);
        assert_eq!(resp["added"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_scale_down_removes_runners() {
        let state = AppState::new_test();
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":3}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let group_id = batch["group_id"].as_str().unwrap();

        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(format!("/runners/groups/{group_id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"count":1}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let resp: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(resp["previous_count"].as_u64().unwrap(), 3);
        assert_eq!(resp["actual_count"].as_u64().unwrap(), 1);
        assert_eq!(resp["removed"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_runners_filter_by_group_id() {
        let state = AppState::new_test();
        // Create a batch
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":2}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let group_id = batch["group_id"].as_str().unwrap();

        // Create a solo runner
        let app = create_router(state.clone());
        app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/runners")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"repo_full_name":"owner/repo","name":"solo-runner"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

        // Filter by group_id
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/runners?group_id={group_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(runners.len(), 2);
    }

    #[tokio::test]
    async fn test_group_delete_removes_runners() {
        let state = AppState::new_test();
        // Create batch
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"repo_full_name":"owner/repo","count":2}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let batch: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let group_id = batch["group_id"].as_str().unwrap();

        // Delete the group
        let app = create_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/runners/groups/{group_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Verify runners are gone
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
}
