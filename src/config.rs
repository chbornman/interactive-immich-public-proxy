use std::env;

/// Runtime configuration, sourced from environment variables.
#[derive(Clone, Debug)]
pub struct Config {
    /// Internal Immich API base, e.g. http://immich_server:2283
    pub immich_url: String,
    /// Public base URL of this service, e.g. https://photos.example.com
    pub public_base_url: String,
    /// Secret used to sign visitor cookies (HMAC-SHA256).
    pub cookie_secret: Vec<u8>,
    /// Path to the SQLite database file.
    pub db_path: String,
    /// Directory containing the built Svelte frontend (index.html + assets/).
    pub web_dir: String,
    /// Bind address, e.g. 0.0.0.0:3000
    pub bind: String,
    /// How long (seconds) before a tenant's cached asset list is considered stale.
    pub sync_ttl_secs: i64,
    /// Max note length in characters.
    pub max_note_len: usize,
    /// Max number of assets allowed in a single download request.
    pub max_download_count: usize,
    /// Max total bytes allowed in a single download request.
    pub max_download_bytes: u64,
}

fn var(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Parse SYNC_TTL_SECS, clamped to strictly positive: a 0/negative TTL would
/// mark every tenant perpetually stale and trigger endless resyncs.
pub(crate) fn parse_sync_ttl(raw: &str) -> i64 {
    raw.parse().ok().filter(|v| *v > 0).unwrap_or(3600)
}

impl Config {
    pub fn from_env() -> Self {
        let cookie_secret = match env::var("COOKIE_SECRET") {
            Ok(s) if s.len() >= 16 => s.into_bytes(),
            _ => {
                tracing::warn!(
                    "COOKIE_SECRET unset or too short; using an ephemeral secret \
                     (visitor cookies will not survive a restart)"
                );
                uuid::Uuid::new_v4().as_bytes().to_vec()
            }
        };

        Config {
            immich_url: var("IMMICH_URL", "http://immich_server:2283")
                .trim_end_matches('/')
                .to_string(),
            public_base_url: var("PUBLIC_BASE_URL", "http://localhost:3000")
                .trim_end_matches('/')
                .to_string(),
            cookie_secret,
            db_path: var("DB_PATH", "/data/ipp.db"),
            web_dir: var("WEB_DIR", "web/dist"),
            bind: var("BIND", "0.0.0.0:3000"),
            sync_ttl_secs: parse_sync_ttl(&var("SYNC_TTL_SECS", "3600")),
            max_note_len: var("MAX_NOTE_LEN", "2000").parse().unwrap_or(2000),
            max_download_count: var("MAX_DOWNLOAD_COUNT", "300").parse().unwrap_or(300),
            max_download_bytes: var("MAX_DOWNLOAD_BYTES", "2147483648")
                .parse()
                .unwrap_or(2_147_483_648),
        }
    }
}
