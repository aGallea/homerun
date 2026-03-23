use crate::server::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::runner::steps::StepsResponse;

#[derive(serde::Serialize)]
pub struct StepLogsResponse {
    pub step_number: u16,
    pub step_name: String,
    pub lines: Vec<String>,
}

pub async fn get_steps(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StepsResponse>, StatusCode> {
    state
        .runner_manager
        .get_steps(&id)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn get_step_logs(
    State(state): State<AppState>,
    Path((id, step_number)): Path<(String, u16)>,
) -> Result<Json<StepLogsResponse>, StatusCode> {
    state
        .runner_manager
        .get_step_logs(&id, step_number)
        .await
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_steps_returns_404_for_unknown_runner() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/unknown-runner-id/steps")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_step_logs_returns_404_for_unknown_runner() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/unknown-runner-id/steps/1/logs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
