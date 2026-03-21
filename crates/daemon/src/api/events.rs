use axum::{
    extract::ws::{Message, WebSocket},
    extract::{State, WebSocketUpgrade},
    response::Response,
};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::server::AppState;

pub async fn events_ws(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let rx = state.runner_manager.subscribe_events();
    let mut stream = BroadcastStream::new(rx);

    while let Some(Ok(event)) = stream.next().await {
        let json = serde_json::to_string(&event).unwrap_or_default();
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_events_endpoint_without_upgrade_returns_400() {
        // Without a WebSocket upgrade header the server should reject with 400
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
