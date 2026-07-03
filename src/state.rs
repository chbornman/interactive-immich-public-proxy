use std::sync::Arc;

use sqlx::SqlitePool;

use crate::config::Config;
use crate::immich::ImmichClient;
use crate::ratelimit::RateLimiter;

/// Shared application state, cloned cheaply (Arc inside).
#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub db: SqlitePool,
    pub immich: ImmichClient,
    pub limiter: Arc<RateLimiter>,
    /// index.html, read once at startup and served from memory.
    pub index_html: String,
    /// Tenant ids with a background resync in flight (thundering-herd guard).
    pub syncing: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
}

/// Resolved visitor identity, injected into request extensions by middleware.
#[derive(Clone, Debug)]
pub struct Visitor {
    pub id: String,
    /// tenant ids this visitor has unlocked (valid signed unlock cookies).
    pub unlocked: std::collections::HashSet<String>,
}

pub fn now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
