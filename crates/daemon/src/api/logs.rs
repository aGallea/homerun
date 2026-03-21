use crate::runner::LogEntry;
use crate::server::AppState;
use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use futures::stream::Stream;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

pub async fn stream_logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.runner_manager.subscribe_logs();
    let stream = BroadcastStream::new(rx).filter_map(move |entry| match entry {
        Ok(log) if log.runner_id == id => {
            let json = serde_json::to_string(&log).unwrap_or_default();
            Some(Ok(Event::default().data(json)))
        }
        _ => None,
    });
    Sse::new(stream)
}

pub async fn recent_logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<Vec<LogEntry>> {
    let logs = state.runner_manager.get_recent_logs(&id).await;
    Json(logs)
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_stream_logs_returns_200() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/test-id/logs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_recent_logs_returns_200_with_empty_array_for_unknown_runner() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/runners/unknown-runner-id/logs/recent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json, serde_json::json!([]));
    }
}
