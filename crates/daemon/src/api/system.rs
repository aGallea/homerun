use axum::Json;

use crate::runner::docker;

#[derive(serde::Serialize)]
pub struct DockerStatusResponse {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Preflight check the frontend uses to decide whether to offer the
/// "Container" runner mode at all — fails closed with a clear message
/// rather than letting a user hit a confusing error mid-creation.
pub async fn docker_status() -> Json<DockerStatusResponse> {
    match docker::docker_status().await {
        Ok(()) => Json(DockerStatusResponse {
            available: true,
            error: None,
        }),
        Err(e) => Json(DockerStatusResponse {
            available: false,
            error: Some(e.to_string()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_docker_status_returns_ok() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/system/docker-status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Always 200 — availability is reported in the body, not the status code.
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_docker_status_response_is_valid_json() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/system/docker-status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["available"].is_boolean());
    }

    #[test]
    fn test_docker_status_response_serialization_omits_error_when_available() {
        let resp = super::DockerStatusResponse {
            available: true,
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("error"));
    }
}
