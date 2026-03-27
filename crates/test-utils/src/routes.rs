use crate::SharedState;
use axum::Router;

pub fn create_router(_state: SharedState) -> Router {
    Router::new()
}
