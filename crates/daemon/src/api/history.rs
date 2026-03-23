use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::github::GitHubClient;
use crate::runner::types::JobHistoryEntry;
use crate::server::AppState;

pub async fn get_runner_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<JobHistoryEntry>>, (StatusCode, String)> {
    // Verify runner exists
    state
        .runner_manager
        .get(&id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))?;

    let history = state.runner_manager.get_job_history(&id).await;
    Ok(Json(history))
}

#[derive(Debug, Deserialize)]
pub struct RerunRequest {
    pub run_url: String,
}

/// Extract the numeric run ID from a GitHub Actions run URL.
/// e.g. "https://github.com/owner/repo/actions/runs/12345" -> 12345
fn parse_run_id(run_url: &str) -> Option<u64> {
    run_url.rsplit('/').next()?.parse().ok()
}

pub async fn rerun_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RerunRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let runner = state
        .runner_manager
        .get(&id)
        .await
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Runner '{id}' not found")))?;

    let run_id = parse_run_id(&req.run_url)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid run_url".to_string()))?;

    let token = state.auth.token().await.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "No auth token available".to_string(),
        )
    })?;

    let gh = GitHubClient::new(Some(token)).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create GitHub client: {e}"),
        )
    })?;

    gh.rerun_workflow(&runner.config.repo_owner, &runner.config.repo_name, run_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to re-run workflow: {e}"),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_history_runner_not_found() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/nonexistent-id/history")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_history_empty_for_new_runner() {
        let state = AppState::new_test_authenticated();

        // Create a runner
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
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id = runner["config"]["id"].as_str().unwrap();

        // Get history — should be an empty array
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/runners/{id}/history"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let history: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(history.is_array());
        assert_eq!(history.as_array().unwrap().len(), 0);
    }
}
