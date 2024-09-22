use axum::{response::IntoResponse, routing::get};

use crate::AppState;

pub fn routes(app_state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/", get(home))
        .with_state(app_state)
}

async fn home() -> impl IntoResponse {
    "Hello, world!"
}
