mod api;
mod auth;
mod db;
mod osint;
mod payments;
mod config;
mod errors;

use std::net::SocketAddr;
use axum::{Router, middleware};
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::Settings;
use crate::db::Database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "ravens_server=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let settings = Settings::from_env()?;
    let db = Database::connect(&settings.database_url).await?;
    db.run_migrations().await?;

    let state = AppState {
        db: db.clone(),
        settings: settings.clone(),
    };

    let cors = CorsLayer::new()
        .allow_origin(["http://localhost:1420".parse().unwrap(), "tauri://localhost".parse().unwrap()])
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .nest("/api/auth",   api::auth::router(state.clone()))
        .nest("/api/osint",  api::osint::router(state.clone()))
        .nest("/api/user",   api::user::router(state.clone()))
        .nest("/api/pay",    api::payments::router(state.clone()))
        .nest("/api/admin",  api::admin::router(state.clone()))
        .route("/health",    axum::routing::get(health))
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = format!("127.0.0.1:{}", settings.port).parse()?;
    tracing::info!("Ravens Nexus Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn health() -> &'static str { "ok" }

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("ctrl-c handler failed");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler failed")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c     => {},
        _ = terminate  => {},
    }
    tracing::info!("Shutdown signal received, stopping server...");
}

#[derive(Clone)]
pub struct AppState {
    pub db:       Database,
    pub settings: Settings,
}
