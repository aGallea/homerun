use axum::{extract::State, http::StatusCode, Json};

use crate::github::{types::RepoInfo, GitHubClient};
use crate::server::AppState;

pub async fn list_repos(
    State(state): State<AppState>,
) -> Result<Json<Vec<RepoInfo>>, (StatusCode, String)> {
    let token = state.auth.token().await;
    let client = GitHubClient::new(token).map_err(|e| (StatusCode::UNAUTHORIZED, e.to_string()))?;
    let repos = client
        .list_repos()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(repos))
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_list_repos_unauthenticated_returns_401() {
        // No token set → GitHubClient::new(None) → UNAUTHORIZED
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/repos")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
