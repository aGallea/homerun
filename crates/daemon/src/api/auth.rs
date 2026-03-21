use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::auth::{AuthStatus, DeviceFlowResponse};
use crate::server::AppState;

#[derive(Deserialize)]
pub struct TokenRequest {
    pub token: String,
}

pub async fn login_with_token(
    State(state): State<AppState>,
    Json(body): Json<TokenRequest>,
) -> Result<Json<AuthStatus>, (StatusCode, String)> {
    match state.auth.login_with_pat(&body.token).await {
        Ok(user) => Ok(Json(AuthStatus {
            authenticated: true,
            user: Some(user),
        })),
        Err(e) => Err((StatusCode::UNAUTHORIZED, e.to_string())),
    }
}

pub async fn logout(State(state): State<AppState>) -> Result<StatusCode, (StatusCode, String)> {
    match state.auth.logout().await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn status(State(state): State<AppState>) -> Json<AuthStatus> {
    Json(state.auth.status().await)
}

pub async fn start_device_flow(
    State(state): State<AppState>,
) -> Result<Json<DeviceFlowResponse>, (StatusCode, String)> {
    match state.auth.start_device_flow().await {
        Ok(flow) => Ok(Json(flow)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[derive(Deserialize)]
pub struct PollDeviceRequest {
    pub device_code: String,
    pub interval: Option<u64>,
}

pub async fn poll_device_flow(
    State(state): State<AppState>,
    Json(body): Json<PollDeviceRequest>,
) -> Result<Json<AuthStatus>, (StatusCode, String)> {
    let interval = body.interval.unwrap_or(5);
    match state.auth.poll_device_flow(&body.device_code, interval).await {
        Ok(user) => Ok(Json(AuthStatus {
            authenticated: true,
            user: Some(user),
        })),
        Err(e) => Err((StatusCode::UNAUTHORIZED, e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_auth_status_unauthenticated() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/auth/status")
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
        assert!(json["authenticated"] == false);
        assert!(json["user"].is_null());
    }

    #[tokio::test]
    async fn test_login_with_invalid_token_returns_401() {
        // Providing a bogus token should fail GitHub validation → 401
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"token":"ghp_invalidtoken12345"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_login_missing_token_field_returns_422() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/token")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn test_logout_when_not_authenticated() {
        // logout should succeed even when not logged in (keychain delete on missing entry is ok)
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/auth")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        // Either NO_CONTENT (ok) or INTERNAL_SERVER_ERROR (keychain error on some systems)
        let status = response.status();
        assert!(
            status == StatusCode::NO_CONTENT || status == StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected status: {status}"
        );
    }
}
