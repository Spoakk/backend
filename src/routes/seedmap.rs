use axum::{extract::{Query, State}, http::{header, StatusCode}, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::ffi::{self, BiomeGenerator};
use crate::services::mojang::{self, version_to_mc_const};

#[derive(Clone)]
pub struct SeedmapState {
    pub client: reqwest::Client,
}

fn java_string_hash(s: &str) -> i32 {
    let mut h: i32 = 0;
    for c in s.chars() {
        h = h.wrapping_mul(31).wrapping_add(c as i32);
    }
    h
}

fn parse_version(v: &str) -> std::ffi::c_int {
    version_to_mc_const(v).unwrap_or(ffi::MC_1_21)
}

#[derive(Deserialize)]
pub struct TileQuery {
    pub seed: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub z: i32,
    #[serde(default = "default_tile_size")]
    pub size: u32,
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_tile_size() -> u32 { 128 }
fn default_version() -> String { "1.21".to_string() }

// GET /api/seedmap/tile?seed=&x=&z=&size=&version=
pub async fn tile_handler(Query(q): Query<TileQuery>) -> impl IntoResponse {
    let seed_str = q.seed.trim().to_string();
    if seed_str.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "seed too long" }))).into_response();
    }

    let size = q.size.clamp(32, 256) as i32;
    let mc   = parse_version(&q.version);
    let seed: i64 = seed_str.parse()
        .unwrap_or_else(|_| java_string_hash(&seed_str) as i64);

    let result = tokio::task::spawn_blocking(move || {
        let gen = BiomeGenerator::new(mc, seed, ffi::NO_FLAGS);
        gen.get_biomes(q.x >> 2, q.z >> 2, size, size, 4, 320)
    }).await;

    match result {
        Ok(biomes) => {
            let mut bytes = Vec::with_capacity(biomes.len() * 2);
            for id in biomes {
                let clamped = id.clamp(-1, 255) as i16;
                bytes.extend_from_slice(&clamped.to_le_bytes());
            }
            (
                [
                    (header::CONTENT_TYPE, "application/octet-stream"),
                    (header::CACHE_CONTROL, "public, max-age=86400"),
                ],
                bytes,
            ).into_response()
        }
        Err(e) => {
            tracing::error!("tile generation panicked: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "tile generation failed").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct StructuresQuery {
    pub seed: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub z: i32,
    #[serde(default = "default_radius")]
    pub radius: i32,
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_radius() -> i32 { 1024 }

#[derive(Serialize)]
pub struct StructureMarker {
    pub kind: &'static str,
    pub label: &'static str,
    pub color: &'static str,
    pub x: i32,
    pub z: i32,
}

// GET /api/seedmap/structures?seed=&x=&z=&radius=&version=
pub async fn structures_handler(Query(q): Query<StructuresQuery>) -> impl IntoResponse {
    let seed_str = q.seed.trim().to_string();
    if seed_str.len() > 64 {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "seed too long" }))).into_response();
    }

    let mc   = parse_version(&q.version);
    let seed: i64 = seed_str.parse()
        .unwrap_or_else(|_| java_string_hash(&seed_str) as i64);
    let radius = q.radius.clamp(256, 4096);

    let result = tokio::task::spawn_blocking(move || {
        let mut gen = BiomeGenerator::new(mc, seed, ffi::NO_FLAGS);

        let types: &[(std::ffi::c_int, &str, &str, &str)] = &[
            (ffi::VILLAGE,        "village",        "Village",        "#4ade80"),
            (ffi::DESERT_PYRAMID, "desert_pyramid", "Desert Pyramid", "#fbbf24"),
            (ffi::JUNGLE_TEMPLE,  "jungle_temple",  "Jungle Temple",  "#86efac"),
            (ffi::SWAMP_HUT,      "swamp_hut",      "Swamp Hut",      "#a78bfa"),
            (ffi::IGLOO,          "igloo",          "Igloo",          "#e0f2fe"),
            (ffi::MONUMENT,       "monument",       "Monument",       "#38bdf8"),
            (ffi::MANSION,        "mansion",        "Mansion",        "#f87171"),
            (ffi::OUTPOST,        "outpost",        "Outpost",        "#fb923c"),
            (ffi::SHIPWRECK,      "shipwreck",      "Shipwreck",      "#94a3b8"),
            (ffi::OCEAN_RUIN,     "ocean_ruin",     "Ocean Ruin",     "#7dd3fc"),
            (ffi::RUINED_PORTAL,  "ruined_portal",  "Ruined Portal",  "#c084fc"),
            (ffi::ANCIENT_CITY,   "ancient_city",   "Ancient City",   "#f43f5e"),
            (ffi::TRIAL_CHAMBERS, "trial_chambers", "Trial Chambers", "#facc15"),
        ];

        let mut markers: Vec<StructureMarker> = Vec::new();
        for &(stype, kind, label, color) in types {
            for (x, z) in gen.find_structures(stype, q.x, q.z, radius) {
                markers.push(StructureMarker { kind, label, color, x, z });
            }
        }
        markers
    }).await;

    match result {
        Ok(markers) => Json(markers).into_response(),
        Err(e) => {
            tracing::error!("structure search panicked: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "structure search failed").into_response()
        }
    }
}

pub fn router(client: reqwest::Client) -> axum::Router {
    use axum::routing::get;
    let state = Arc::new(SeedmapState { client });
    axum::Router::new()
        .route("/versions", get(versions_handler))
        .route("/tile", get(tile_handler))
        .route("/structures", get(structures_handler))
        .with_state(state)
}

// GET /api/seedmap/versions
async fn versions_handler(State(state): State<Arc<SeedmapState>>) -> impl IntoResponse {
    match mojang::get_supported_versions(&state.client).await {
        Ok(versions) => Json(json!({ "versions": versions })).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}
