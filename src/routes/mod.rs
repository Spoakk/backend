mod serverjars;
mod mcping;
mod player;
mod seedmap;

use axum::{Router, routing::get, Json};
use serde_json::json;

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok"
    }))
}

pub fn router(client: reqwest::Client) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .nest("/serverjars", serverjars::router(client.clone()))
        .nest("/mcping", mcping::router())
        .nest("/seedmap", seedmap::router(client.clone()))
        .nest("/player", player::router(client))
}
