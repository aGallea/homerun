use axum::body::Body;
use axum::http::{Request, StatusCode};
use homerund::config::Config;
use homerund::server::{create_router, AppState};
use tower::ServiceExt;

fn test_state() -> AppState {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::with_base_dir(dir.keep().join(".homerun"));
    config.ensure_dirs().unwrap();
    AppState::new(config)
}

#[tokio::test]
async fn test_health() {
    let state = test_state();
    let app = create_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn test_runner_crud_flow() {
    let state = test_state();

    // Create
    let app = create_router(state.clone());
    let resp = app
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
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = runner["config"]["id"].as_str().unwrap().to_string();

    // List
    let app = create_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/runners")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(runners.len(), 1);

    // Get by ID
    let app = create_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/runners/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Delete
    let app = create_router(state.clone());
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/runners/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify gone
    let app = create_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/runners")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(runners.len(), 0);
}

#[tokio::test]
async fn test_auth_status_unauthenticated() {
    let state = test_state();
    let app = create_router(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(status["authenticated"], false);
}
