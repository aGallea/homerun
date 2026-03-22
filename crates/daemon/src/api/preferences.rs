use axum::{extract::State, http::StatusCode, Json};

use crate::config::Preferences;
use crate::server::AppState;

pub async fn get_preferences(State(state): State<AppState>) -> Json<Preferences> {
    let config = state.config.read().await;
    Json(config.preferences.clone())
}

pub async fn update_preferences(
    State(state): State<AppState>,
    Json(prefs): Json<Preferences>,
) -> Result<Json<Preferences>, (StatusCode, String)> {
    let mut config = state.config.write().await;
    config.preferences = prefs.clone();

    let config_path = config.config_path();
    config
        .save(&config_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // TODO: uncomment after Task 4 adds these methods
    // state.notifications.set_status_changes(prefs.notify_status_changes);
    // state.notifications.set_job_completions(prefs.notify_job_completions);

    Ok(Json(prefs))
}

#[cfg(test)]
mod tests {
    use crate::server::{create_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_preferences_returns_defaults() {
        let app = create_router(AppState::new_test());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/preferences")
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
        assert_eq!(json["start_runners_on_launch"], false);
        assert_eq!(json["notify_status_changes"], true);
        assert_eq!(json["notify_job_completions"], true);
    }

    #[tokio::test]
    async fn test_update_preferences_persists() {
        let state = AppState::new_test();
        let app = create_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/preferences")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"start_runners_on_launch":true,"notify_status_changes":false,"notify_job_completions":true}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["start_runners_on_launch"], true);
        assert_eq!(json["notify_status_changes"], false);
    }
}
