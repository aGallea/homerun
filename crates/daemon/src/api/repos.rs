use axum::{Json, extract::State, http::StatusCode};

use crate::github::{types::RepoInfo, GitHubClient};
use crate::server::AppState;

pub async fn list_repos(
    State(state): State<AppState>,
) -> Result<Json<Vec<RepoInfo>>, (StatusCode, String)> {
    let token = state.auth.token().await;
    let client =
        GitHubClient::new(token).map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let repos = client
        .list_repos()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}
