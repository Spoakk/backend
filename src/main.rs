mod routes;
mod services;
mod models;
mod middleware;
mod ffi;
mod constants;

use axum::{Router, http::Method, Extension};
use tower_http::cors::{CorsLayer, AllowOrigin};
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse};
use std::net::SocketAddr;
use std::time::Duration;
use tracing::Level;
use middleware::Limiters;

#[tokio::main]
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
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(90))
        .timeout(Duration::from_secs(10))
        .user_agent(constants::USER_AGENT)
        .build()?;

    let allowed_origins = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "https://spoak.cc,http://localhost:3000".to_string())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect::<Vec<_>>();
    
    let cors = CorsLayer::new()
        .allow_methods([Method::GET])
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_headers(tower_http::cors::Any);

    let limiters = Limiters::new();

    let app = Router::new()
        .nest("/api", routes::router(http_client))
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

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>()
    ).await?;

    Ok(())
}
