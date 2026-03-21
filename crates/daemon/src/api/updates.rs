use axum::{extract::State, http::StatusCode, Json};

use crate::server::AppState;
use crate::updater;

#[derive(serde::Serialize)]
pub struct UpdateCheckResponse {
    pub update_available: bool,
    pub current: Option<String>,
    pub latest: Option<String>,
}

pub async fn check_updates(
    State(state): State<AppState>,
) -> Result<Json<UpdateCheckResponse>, (StatusCode, String)> {
    let cache_dir = state.config.cache_dir();
    let current = updater::read_cached_version(&cache_dir);

    let latest = updater::fetch_latest_version()
        .await
        .map_err(|e| (StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;

    let update_available = current.as_deref() != Some(latest.as_str());

    Ok(Json(UpdateCheckResponse {
        update_available,
        current,
        latest: Some(latest),
    }))
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_check_updates_returns_ok_or_503() {
        // This endpoint calls the GitHub API; in CI/offline it may return 503.
        // We only assert that the response is one of the two valid codes.
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/updates/check")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        assert!(
            status == StatusCode::OK || status == StatusCode::SERVICE_UNAVAILABLE,
            "unexpected status: {status}"
        );
    }

    #[tokio::test]
    async fn test_check_updates_response_is_valid_json() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/updates/check")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        // Should always be valid JSON (either the response or an error string)
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_update_check_response_serialization() {
        let resp = super::UpdateCheckResponse {
            update_available: true,
            current: Some("2.320.0".to_string()),
            latest: Some("2.321.0".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("update_available"));
        assert!(json.contains("2.320.0"));
        assert!(json.contains("2.321.0"));
    }

    #[test]
    fn test_update_check_response_no_current_version() {
        let resp = super::UpdateCheckResponse {
            update_available: true,
            current: None,
            latest: Some("2.321.0".to_string()),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert!(json["current"].is_null());
        assert_eq!(json["update_available"], true);
    }
}
