mod serverjars;
mod mcping;
mod player;
mod seedmap;

use axum::{Router, routing::get, response::IntoResponse, http::header};

static HEALTH_BODY: &[u8] = br#"{"status":"ok"}"#;

async fn health_check() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        HEALTH_BODY,
    )
}

pub fn router(client: reqwest::Client) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .nest("/serverjars", serverjars::router(client.clone()))
        .nest("/mcping", mcping::router())
        .nest("/seedmap", seedmap::router(client.clone()))
        .nest("/player", player::router(client))
}
