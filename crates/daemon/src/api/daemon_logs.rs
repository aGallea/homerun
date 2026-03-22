use crate::logging::{level_value, DaemonLogEntry};
use crate::server::AppState;
use axum::extract::{Query, State};
use axum::response::sse::{Event, Sse};
use axum::Json;
use futures::stream::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Deserialize)]
pub struct StreamQuery {
    pub level: Option<String>,
}

pub async fn stream_daemon_logs(
    State(state): State<AppState>,
    Query(query): Query<StreamQuery>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.daemon_logs.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(move |entry| {
        let level_filter = query.level.clone();
        async move {
            match entry {
                Ok(log) => {
                    if let Some(ref min_level) = level_filter {
                        if level_value(&log.level) < level_value(min_level) {
                            return None;
                        }
                    }
                    let json = serde_json::to_string(&log).unwrap_or_default();
                    Some(Ok(Event::default().data(json)))
                }
                _ => None,
            }
        }
    });
    Sse::new(stream)
}

#[derive(Deserialize)]
pub struct RecentQuery {
    pub level: Option<String>,
    pub limit: Option<usize>,
    pub search: Option<String>,
}

pub async fn recent_daemon_logs(
    State(state): State<AppState>,
    Query(query): Query<RecentQuery>,
) -> Json<Vec<DaemonLogEntry>> {
    let limit = query.limit.unwrap_or(500).min(2000);
    let entries = state
        .daemon_logs
        .get_recent(query.level.as_deref(), limit, query.search.as_deref())
        .await;
    Json(entries)
}

#[cfg(test)]
mod tests {
    use crate::logging::DaemonLogEntry;
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use chrono::Utc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_stream_daemon_logs_returns_200() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/daemon/logs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_recent_daemon_logs_returns_200_with_empty_array() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/daemon/logs/recent")
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

    #[tokio::test]
    async fn test_stream_daemon_logs_returns_sse_content_type() {
        let state = AppState::new_test();
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/daemon/logs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .expect("should have content-type header")
            .to_str()
            .unwrap();
        assert!(
            content_type.contains("text/event-stream"),
            "SSE endpoint should return text/event-stream, got: {}",
            content_type
        );
    }

    #[tokio::test]
    async fn test_recent_daemon_logs_with_entries() {
        let state = AppState::new_test();

        // Push some entries into the daemon log state
        state
            .daemon_logs
            .push(DaemonLogEntry {
                timestamp: Utc::now(),
                level: "INFO".to_string(),
                target: "test".to_string(),
                message: "test entry".to_string(),
            })
            .await;

        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/daemon/logs/recent")
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
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["message"], "test entry");
        assert_eq!(arr[0]["level"], "INFO");
    }
}
