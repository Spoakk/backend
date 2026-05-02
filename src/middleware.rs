use axum::{
    extract::{Request, ConnectInfo},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use dashmap::DashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::time::interval;

static RATE_LIMIT_BODY: &[u8] = br#"{"error":"Rate limit exceeded","message":"Too many requests. Please wait a moment before trying again.","retry_after":60}"#;

#[inline(always)]
fn is_trusted_proxy(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
            || o[0] == 10
            || (o[0] == 172 && o[1] >= 16 && o[1] <= 31)
            || (o[0] == 192 && o[1] == 168)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 (ULA)
        }
    }
}

struct AtomicWindow {
    count: AtomicU32,
    window_start_ms: AtomicU64,
}

impl AtomicWindow {
    fn new() -> Self {
        Self {
            count: AtomicU32::new(0),
            window_start_ms: AtomicU64::new(0),
        }
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    windows: Arc<DashMap<IpAddr, Arc<AtomicWindow>>>,
    max_requests: u32,
    window_ms: u64,
    epoch: Instant,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        let epoch = Instant::now();
        let limiter = Self {
            windows: Arc::new(DashMap::with_capacity_and_hasher(256, Default::default())),
            max_requests: max_requests as u32,
            window_ms: window.as_millis() as u64,
            epoch,
        };

        let map = limiter.windows.clone();
        let win_ms = limiter.window_ms;
        let ep = epoch;
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(120));
            loop {
                ticker.tick().await;
                let now_ms = ep.elapsed().as_millis() as u64;
                map.retain(|_, w: &mut Arc<AtomicWindow>| {
                    let ws = w.window_start_ms.load(Ordering::Relaxed);
                    now_ms.saturating_sub(ws) < win_ms * 2
                });
            }
        });

        limiter
    }

    #[inline(always)]
    pub fn check(&self, ip: IpAddr) -> bool {
        let now_ms = self.epoch.elapsed().as_millis() as u64;

        let window = self.windows
            .entry(ip)
            .or_insert_with(|| Arc::new(AtomicWindow::new()))
            .clone();

        let ws = window.window_start_ms.load(Ordering::Relaxed);

        if now_ms.saturating_sub(ws) >= self.window_ms
            && window.window_start_ms.compare_exchange(
                ws, now_ms, Ordering::AcqRel, Ordering::Relaxed
            ).is_ok()
        {
            window.count.store(1, Ordering::Release);
            return true;
        }

        let prev = window.count.fetch_add(1, Ordering::AcqRel);
        if prev >= self.max_requests {
            window.count.fetch_sub(1, Ordering::Relaxed);
            return false;
        }
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
            general: RateLimiter::new(60, Duration::from_secs(60)),
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

    if path.starts_with("/api/v2/health") || path.ends_with("/health") {
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

    let limiter = if path.starts_with("/api/v2/seedmap") {
        &limiters.seedmap
    } else if path.starts_with("/api/v2/serverjars") {
        &limiters.serverjars
    } else {
        &limiters.general
    };

    if !limiter.check(ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            RATE_LIMIT_BODY,
        ).into_response();
    }

    next.run(req).await
}
