use axum::{Json, extract::State};

use crate::server::AppState;

pub async fn get_metrics(State(state): State<AppState>) -> Json<serde_json::Value> {
    let system = state.metrics.system_snapshot();
    let runners = state.runner_manager.list().await;
    let runner_metrics: Vec<_> = runners
        .iter()
        .filter_map(|r| {
            r.pid.and_then(|pid| {
                state.metrics.runner_metrics(pid).map(|mut m| {
                    m.runner_id = r.config.id.clone();
                    m
                })
            })
        })
        .collect();
    Json(serde_json::json!({ "system": system, "runners": runner_metrics }))
}
