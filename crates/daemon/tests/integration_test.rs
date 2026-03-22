use axum::body::Body;
use axum::http::{Request, StatusCode};
use homerund::config::Config;
use homerund::server::{create_router, AppState};
use tower::ServiceExt;

fn test_state() -> AppState {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::with_base_dir(dir.keep().join(".homerun"));
    config.ensure_dirs().unwrap();
    let mut state = AppState::new(config);
    state.auth = homerund::auth::AuthManager::new_test_authenticated();
    state
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

/// Create 3 runners for the same repo in a single shared state (simulating the
/// multiple-runner batch creation that the NewRunnerWizard performs) and verify:
///   - all 3 requests succeed with HTTP 201
///   - all 3 runners appear in the list
///   - all 3 IDs are unique
///   - names follow the "{repo}-runner-{n}" pattern the daemon generates
#[tokio::test]
async fn test_create_multiple_runners_for_same_repo() {
    let state = test_state();
    let repo = "owner/myrepo";
    let mut ids = Vec::new();
    let mut names = Vec::new();

    for _ in 0..3 {
        let app = create_router(state.clone());
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/runners")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({ "repo_full_name": repo }).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED, "runner creation failed");

        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let runner: serde_json::Value = serde_json::from_slice(&body).unwrap();
        ids.push(runner["config"]["id"].as_str().unwrap().to_string());
        names.push(runner["config"]["name"].as_str().unwrap().to_string());
    }

    // All IDs must be unique
    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique_ids.len(), 3, "runner IDs must all be unique");

    // Names must follow the pattern "myrepo-runner-N"
    for (i, name) in names.iter().enumerate() {
        assert!(
            name.starts_with("myrepo-runner-"),
            "runner name '{}' does not match expected pattern 'myrepo-runner-N'",
            name
        );
        let n: usize = name
            .strip_prefix("myrepo-runner-")
            .unwrap()
            .parse()
            .expect("suffix should be a number");
        assert_eq!(
            n,
            i + 1,
            "runner name suffix should be sequential (1-based)"
        );
    }

    // List must return exactly 3 runners
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
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let runners: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(runners.len(), 3, "expected 3 runners in the list");
}

#[tokio::test]
async fn test_auth_status_unauthenticated() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::with_base_dir(dir.keep().join(".homerun"));
    config.ensure_dirs().unwrap();
    let state = AppState::new(config);
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
    assert!(status["authenticated"] == false);
}
