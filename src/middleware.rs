use axum::{
    extract::{Request, ConnectInfo},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::interval;

fn is_trusted_proxy(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
            || o[0] == 10
            || (o[0] == 172 && o[1] >= 16 && o[1] <= 31)
            || (o[0] == 192 && o[1] == 168)
        }
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    requests: Arc<DashMap<IpAddr, Vec<Instant>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        let limiter = Self {
            requests: Arc::new(DashMap::new()),
            max_requests,
            window,
        };
        let map = limiter.requests.clone();
        let win = window;
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(120));
            loop {
                ticker.tick().await;
                let now = Instant::now();
                map.retain(|_, times: &mut Vec<Instant>| {
                    times.retain(|&t| now.duration_since(t) < win);
                    !times.is_empty()
                });
            }
        });
        limiter
    }

    pub fn check(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.requests.entry(ip).or_default();
        entry.retain(|&t| now.duration_since(t) < self.window);
        if entry.len() >= self.max_requests {
            return false;
        }
        entry.push(now);
        true
    }
}

#[derive(Clone)]
pub struct Limiters {
    pub general: RateLimiter,
    pub seedmap: RateLimiter,
    pub serverjars: RateLimiter,
}

impl Limiters {
    pub fn new() -> Self {
        Self {
            general: RateLimiter::new(10, Duration::from_secs(60)),
            seedmap: RateLimiter::new(120, Duration::from_secs(60)),
            serverjars: RateLimiter::new(60, Duration::from_secs(60)),
        }
    }
}

pub async fn rate_limit_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();

    if path == "/api/health" {
        return next.run(req).await;
    }

    let limiters = req.extensions().get::<Limiters>().cloned();
    let Some(limiters) = limiters else {
        return next.run(req).await;
    };

    let ip = if is_trusted_proxy(addr.ip()) {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse::<IpAddr>().ok())
            .unwrap_or_else(|| addr.ip())
    } else {
        addr.ip()
    };

    let limiter = if path.starts_with("/api/seedmap") {
        &limiters.seedmap
    } else if path.starts_with("/api/serverjars") {
        &limiters.serverjars
    } else {
        &limiters.general
    };

    if !limiter.check(ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(serde_json::json!({
                "error": "Rate limit exceeded",
                "message": "Too many requests. Please wait a moment before trying again.",
                "retry_after": 60
            }))
        ).into_response();
    }

    next.run(req).await
}
