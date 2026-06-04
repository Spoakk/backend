mod routes;
mod services;
mod models;
mod middleware;
mod ffi;
mod constants;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use axum::{Router, http::Method, Extension};
use tower_http::cors::{CorsLayer, AllowOrigin};
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse};
use tower_http::compression::CompressionLayer;
use std::time::Duration;
use tracing::Level;
use middleware::Limiters;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let _sentry = std::env::var("SENTRY_DSN").ok().map(|dsn| {
        sentry::init((dsn, sentry::ClientOptions {
            release: sentry::release_name!(),
            send_default_pii: false,
            ..Default::default()
        }))
    });

    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let http_client = reqwest::Client::builder()
        .pool_max_idle_per_host(100)
        .pool_idle_timeout(Duration::from_secs(180))
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(5))
        .tcp_keepalive(Duration::from_secs(60))
        .tcp_nodelay(true)
        // .http2_prior_knowledge() // REMOVED: causes HTTPS connections to fail
        .http2_keep_alive_interval(Duration::from_secs(20))
        .http2_keep_alive_timeout(Duration::from_secs(10))
        .http2_keep_alive_while_idle(true)
        .http2_adaptive_window(true)
        .http2_max_frame_size(Some(16384))
        .gzip(true)
        .brotli(true)
        .deflate(true)
        .user_agent(constants::USER_AGENT)
        .build()?;

    let allowed_origins = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "https://spoak.cc,http://localhost:3000,https://spoak.vercel.app".to_string())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect::<Vec<_>>();
    
    let cors = CorsLayer::new()
        .allow_methods([Method::GET])
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_headers(tower_http::cors::Any);

    let limiters = Limiters::new();

    let compression = CompressionLayer::new()
        .gzip(true)
        .br(true)
        .zstd(true)
        .deflate(true);

    let app = Router::new()
        .nest("/api/v2", routes::router(http_client))
        .layer(compression)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO))
        )
        .layer(axum::middleware::from_fn(middleware::rate_limit_middleware))
        .layer(Extension(limiters))
        .layer(cors);

    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "4000".into())
        .parse()?;

    let addr = std::net::SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, port));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>()
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown...");
}
