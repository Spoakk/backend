use axum::{Router, routing::get, extract::Query, Json, response::IntoResponse, http::StatusCode};
use serde::Deserialize;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};

use crate::services::mcping;

#[derive(Deserialize)]
pub struct PingQuery {
    host: String,
    port: Option<u16>,
}

pub fn router() -> Router {
    Router::new().route("/", get(ping_handler))
}

fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let o = ipv4.octets();
            ipv4.is_loopback()
                || ipv4.is_multicast()
                || ipv4.is_link_local()
                || o[0] == 10
                || (o[0] == 172 && o[1] >= 16 && o[1] <= 31)
                || (o[0] == 192 && o[1] == 168)
                || (o[0] == 169 && o[1] == 254)
                || o[0] == 0
                || o[0] == 127
                || o[0] == 100 && (o[1] >= 64 && o[1] <= 127)
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_multicast()
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

async fn resolve_and_validate(host: &str, port: u16) -> Result<SocketAddr, &'static str> {
    let lower = host.to_lowercase();
    if lower == "localhost"
        || lower == "metadata.google.internal"
        || lower == "metadata"
        || lower == "instance-data"
        || lower == "computemetadata"
        || lower.ends_with(".internal")
        || lower.ends_with(".local")
    {
        return Err("Forbidden host");
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            return Err("Private IP addresses not allowed");
        }
        return Ok(SocketAddr::new(ip, port));
    }

    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(format!("{}:{}", host, port))
        .await
        .map_err(|_| "Could not resolve host")?
        .collect();

    if addrs.is_empty() {
        return Err("Host resolved to no addresses");
    }

    for addr in &addrs {
        if is_private_ip(&addr.ip()) {
            return Err("Host resolves to a private IP address");
        }
    }

    Ok(addrs[0])
}

// GET /api/mcping?host=play.example.com&port=25565
async fn ping_handler(Query(q): Query<PingQuery>) -> impl IntoResponse {
    let host = q.host.trim().to_string();
    if host.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "host is required" })),
        ).into_response();
    }

    let port = q.port.unwrap_or(25565);
    if port == 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid port (must be 1-65535)" })),
        ).into_response();
    }

    match resolve_and_validate(&host, port).await {
        Err(reason) => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": reason })),
        ).into_response(),
        Ok(resolved_addr) => {
            let status = mcping::ping_addr(&host, resolved_addr).await;
            Json(status).into_response()
        }
    }
}
