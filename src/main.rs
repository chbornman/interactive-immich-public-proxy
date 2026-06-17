mod config;
mod db;
mod error;
mod immich;
mod ratelimit;
mod routes;
mod state;
mod tenant;
mod visitor;
mod web;

use std::sync::Arc;

use crate::config::Config;
use crate::immich::ImmichClient;
use crate::ratelimit::RateLimiter;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let cfg = Config::from_env();
    tracing::info!(
        "starting interactive-ipp: immich_url={} db={} web_dir={}",
        cfg.immich_url,
        cfg.db_path,
        cfg.web_dir
    );

    // Ensure the DB directory exists.
    if let Some(parent) = std::path::Path::new(&cfg.db_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let db = db::init(&cfg.db_path).await?;
    let immich = ImmichClient::new(&cfg.immich_url);
    let limiter = Arc::new(RateLimiter::new(60.0, 15.0));
    let bind = cfg.bind.clone();

    let state = AppState {
        cfg: Arc::new(cfg),
        db,
        immich,
        limiter,
    };

    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("listening on http://{bind}");
    axum::serve(listener, app).await?;
    Ok(())
}
