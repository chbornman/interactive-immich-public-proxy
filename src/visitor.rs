use axum::extract::{Request, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use sha2::{Digest, Sha256};

use crate::state::{now, AppState, Visitor};

/// HMAC-SHA256 (RFC 2104), implemented directly on sha2 to avoid an extra crate.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let inner = Sha256::digest([&ipad[..], msg].concat());
    let outer = Sha256::digest([&opad[..], &inner[..]].concat());
    let mut out = [0u8; 32];
    out.copy_from_slice(&outer);
    out
}

pub(crate) fn sign(secret: &[u8], id: &str) -> String {
    hex::encode(hmac_sha256(secret, id.as_bytes()))
}

/// Cookie value is `<uuid>.<hmac>`; uuid never contains '.'.
pub(crate) fn make_cookie_value(secret: &[u8], id: &str) -> String {
    format!("{id}.{}", sign(secret, id))
}

pub(crate) fn verify(secret: &[u8], value: &str) -> Option<String> {
    let (id, sig) = value.split_once('.')?;
    if sign(secret, id) == sig {
        Some(id.to_string())
    } else {
        None
    }
}

/// Signature proving a visitor unlocked a password-protected tenant.
fn unlock_sig(secret: &[u8], tenant_id: &str) -> String {
    hex::encode(hmac_sha256(secret, format!("unlock:{tenant_id}").as_bytes()))
}

/// Set-Cookie value granting this visitor access to a password-protected tenant.
pub fn unlock_set_cookie(secret: &[u8], tenant_id: &str) -> String {
    format!(
        "u_{tenant_id}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000",
        unlock_sig(secret, tenant_id)
    )
}

fn parse_cookies(req: &Request) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(h) = req.headers().get(COOKIE).and_then(|v| v.to_str().ok()) {
        for part in h.split(';') {
            if let Some((k, v)) = part.trim().split_once('=') {
                out.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    out
}

/// Middleware that ensures every request has a stable, signed visitor id.
/// Injects `Visitor` into request extensions and sets the cookie if newly minted.
pub async fn middleware(State(st): State<AppState>, mut req: Request, next: Next) -> Response {
    let cookies = parse_cookies(&req);
    let existing = cookies
        .iter()
        .find(|(k, _)| k == "vid")
        .and_then(|(_, v)| verify(&st.cfg.cookie_secret, v));

    let (id, fresh) = match existing {
        Some(id) => (id, false),
        None => (uuid::Uuid::new_v4().to_string(), true),
    };

    // Collect tenants this visitor has unlocked (valid signed unlock cookies).
    let mut unlocked = std::collections::HashSet::new();
    for (k, v) in &cookies {
        if let Some(tid) = k.strip_prefix("u_") {
            if unlock_sig(&st.cfg.cookie_secret, tid) == *v {
                unlocked.insert(tid.to_string());
            }
        }
    }

    // Insert the visitor row only when the id is freshly minted. Returning
    // visitors already have a row (rows are never deleted; ban/set-name are
    // UPDATEs), so we skip the write on every media byte fetch.
    if fresh {
        let _ = sqlx::query("INSERT OR IGNORE INTO visitor (id, first_seen) VALUES (?, ?)")
            .bind(&id)
            .bind(now())
            .execute(&st.db)
            .await;
    }

    req.extensions_mut().insert(Visitor {
        id: id.clone(),
        unlocked,
    });

    let mut resp = next.run(req).await;

    if fresh {
        let cookie = format!(
            "vid={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000",
            make_cookie_value(&st.cfg.cookie_secret, &id)
        );
        if let Ok(hv) = HeaderValue::from_str(&cookie) {
            resp.headers_mut().append(SET_COOKIE, hv);
        }
    }
    resp
}

/// Fetch a visitor's display name (empty string if unset).
pub async fn display_name(db: &sqlx::SqlitePool, id: &str) -> String {
    sqlx::query_as::<_, (String,)>("SELECT display_name FROM visitor WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
        .unwrap_or_default()
}

/// Whether a visitor is banned.
pub async fn is_banned(db: &sqlx::SqlitePool, id: &str) -> bool {
    sqlx::query_as::<_, (i64,)>("SELECT banned FROM visitor WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0 != 0)
        .unwrap_or(false)
}
