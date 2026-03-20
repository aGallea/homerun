use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde::Deserialize;

use crate::auth::AuthStatus;
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

pub async fn logout(
    State(state): State<AppState>,
) -> Result<StatusCode, (StatusCode, String)> {
    match state.auth.logout().await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn status(State(state): State<AppState>) -> Json<AuthStatus> {
    Json(state.auth.status().await)
}
