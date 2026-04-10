use axum::{Router, routing::get, extract::{Path, State}, Json, http::StatusCode, response::IntoResponse};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use moka::future::Cache;

use crate::services::playerprofile::{self, PlayerProfile};

#[derive(Clone)]
pub struct AppState {
    pub client: reqwest::Client,
    pub profile_cache: Cache<String, PlayerProfile>,
}

pub fn router(client: reqwest::Client) -> Router {
    let state = Arc::new(AppState {
        client,
        profile_cache: Cache::builder()
            .time_to_live(Duration::from_secs(300)) // 5 min TTL
            .max_capacity(500)
            .build(),
    });
    Router::new()
        .route("/:username", get(get_player))
        .with_state(state)
}

// GET /api/player/:username
async fn get_player(
    State(state): State<Arc<AppState>>,
    Path(username): Path<String>,
) -> impl IntoResponse {
    let username = username.trim().to_string();
    if username.is_empty() || username.len() > 16 || username.len() < 3 ||
       !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid username format" })),
        ).into_response();
    }

    let key = username.to_lowercase();
    let result: Result<PlayerProfile, _> = state.profile_cache
        .try_get_with(key, playerprofile::get_profile(&state.client, &username))
        .await;
    match result {
        Ok(profile) => Json(profile).into_response(),
        Err(e) => {
            let msg = e.to_string();
            let status = if msg.contains("not found") { StatusCode::NOT_FOUND } else { StatusCode::BAD_GATEWAY };
            (status, Json(json!({ "error": msg }))).into_response()
        }
    }
}
