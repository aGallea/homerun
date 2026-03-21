use crate::server::AppState;
use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
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
}
