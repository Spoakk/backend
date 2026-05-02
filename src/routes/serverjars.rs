use axum::{
    Router,
    routing::get,
    extract::{Path, State},
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use moka::future::Cache;

use crate::services::{paper, leaf, mojang};
use crate::models::JarVersion;

fn validate_version(version: &str) -> Result<(), StatusCode> {
    if version.is_empty() || version.len() > 20 || 
       !version.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

#[derive(Clone)]
pub struct AppState {
    pub client: reqwest::Client,
    pub version_cache: Cache<String, Vec<String>>,
    pub builds_cache: Cache<String, Vec<JarVersion>>,
}

pub fn router(client: reqwest::Client) -> Router {
    let version_cache = Cache::builder()
        .time_to_live(Duration::from_secs(7200))
        .max_capacity(20)
        .build();

    let builds_cache = Cache::builder()
        .time_to_live(Duration::from_secs(3600))
        .max_capacity(2000)
        .build();

    let state = Arc::new(AppState {
        client,
        version_cache,
        builds_cache,
    });

    Router::new()
        // Mojang'dan gelen canonical versiyon listesi
        .route("/versions", get(mc_versions))
        // Paper
        .route("/paper/:version/builds", get(paper_builds))
        .route("/paper/:version/latest", get(paper_latest))
        // Leaf
        .route("/leaf/:version/builds", get(leaf_builds))
        .with_state(state)
}

// GET /api/serverjars/versions — Mojang release versiyonları
async fn mc_versions(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let cache_key = "mojang_versions".to_string();
    
    match state.version_cache
        .try_get_with(cache_key, async {
            mojang::get_release_versions(&state.client).await
        })
        .await
    {
        Ok(versions) => (
            StatusCode::OK,
            [(axum::http::header::CACHE_CONTROL, "public, max-age=7200")],
            Json(json!({ "versions": versions }))
        ).into_response(),
        Err(e) => {
            tracing::error!("Mojang version fetch failed: {e:#}");
            (StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() }))).into_response()
        },
    }
}

async fn paper_builds(
    State(state): State<Arc<AppState>>,
    Path(version): Path<String>,
) -> impl IntoResponse {
    if let Err(status) = validate_version(&version) {
        return (status, Json(json!({ "error": "Invalid version format" }))).into_response();
    }
    
    let cache_key = format!("paper_{}", version);
    
    match state.builds_cache
        .try_get_with(cache_key, async {
            paper::get_builds(&state.client, &version).await
        })
        .await
    {
        Ok(builds) => (
            StatusCode::OK,
            [(axum::http::header::CACHE_CONTROL, "public, max-age=3600")],
            Json(json!({ "builds": builds }))
        ).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn paper_latest(
    State(state): State<Arc<AppState>>,
    Path(version): Path<String>,
) -> impl IntoResponse {
    if let Err(status) = validate_version(&version) {
        return (status, Json(json!({ "error": "Invalid version format" }))).into_response();
    }

    let cache_key = format!("paper_{}", version);
    let builds_result = state.builds_cache
        .try_get_with(cache_key, async {
            paper::get_builds(&state.client, &version).await
        })
        .await;

    match builds_result {
        Ok(builds) => {
            let latest = builds.iter()
                .find(|b| b.channel == "stable")
                .or_else(|| builds.iter().find(|b| b.channel == "experimental"))
                .cloned();
            match latest {
                Some(build) => (
                    StatusCode::OK,
                    [(axum::http::header::CACHE_CONTROL, "public, max-age=3600")],
                    Json(build)
                ).into_response(),
                None => (StatusCode::NOT_FOUND, Json(json!({ "error": format!("No build found for {version}") }))).into_response(),
            }
        }
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

async fn leaf_builds(
    State(state): State<Arc<AppState>>,
    Path(version): Path<String>,
) -> impl IntoResponse {
    if let Err(status) = validate_version(&version) {
        return (status, Json(json!({ "error": "Invalid version format" }))).into_response();
    }
    
    let cache_key = format!("leaf_{}", version);
    
    match state.builds_cache
        .try_get_with(cache_key, async {
            leaf::get_builds_for_version(&state.client, &version).await
        })
        .await
    {
        Ok(builds) => (
            StatusCode::OK,
            [(axum::http::header::CACHE_CONTROL, "public, max-age=3600")],
            Json(json!({ "builds": builds }))
        ).into_response(),
        Err(e) => (StatusCode::NOT_FOUND, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
