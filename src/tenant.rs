use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::error::{AppError, AppResult};
use crate::state::{now, AppState, Visitor};

/// Tenant settings row used by handlers.
#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Tenant {
    pub id: String,
    pub title: Option<String>,
    pub allow_marks: bool,
    pub allow_notes: bool,
    pub allow_download: bool,
    pub public_write: bool,
    pub needs_password: bool,
    pub share_token: String,
}

impl Tenant {
    /// immich_shared_link_token for password-protected albums (None if open).
    pub fn token(&self) -> Option<&str> {
        if self.share_token.is_empty() {
            None
        } else {
            Some(self.share_token.as_str())
        }
    }
}

#[derive(sqlx::FromRow)]
struct TenantSync {
    immich_album: Option<String>,
    synced_at: Option<i64>,
    needs_password: bool,
    share_token: String,
}

/// Stable, non-reversible tenant id derived from the share key.
pub fn tenant_id(key: &str) -> String {
    let mut h = Sha256::new();
    h.update(key.as_bytes());
    hex::encode(h.finalize())
}

/// Password gate: a password-protected album requires the visitor to have unlocked it.
pub fn gate(t: &Tenant, v: &Visitor) -> AppResult<()> {
    if t.needs_password && !v.unlocked.contains(&t.id) {
        return Err(AppError::PasswordRequired);
    }
    Ok(())
}

/// Ensure a tenant exists, validating the share key against Immich on first sight.
/// Returns `PasswordRequired` if the album's Immich share needs a password we
/// don't have a token for yet (the SPA then prompts and calls `unlock`).
pub async fn ensure(st: &AppState, key: &str) -> AppResult<Tenant> {
    let id = tenant_id(key);

    let existing: Option<TenantSync> = sqlx::query_as(
        "SELECT immich_album, synced_at, needs_password, share_token FROM tenant WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&st.db)
    .await?;

    match existing {
        None => match st.immich.share_info(key, None).await {
            Ok(info) => {
                sqlx::query(
                    "INSERT OR IGNORE INTO tenant \
                     (id, share_key, immich_album, title, allow_download, created_at) \
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&id)
                .bind(key)
                .bind(&info.album_id)
                .bind(&info.title)
                .bind(info.allow_download)
                .bind(now())
                .execute(&st.db)
                .await?;
                sync_assets(&st.db, &st.immich, &id, &info.album_id, key, None).await?;
            }
            Err(AppError::PasswordRequired) => {
                // Record that this album needs a password; can't sync until unlocked.
                sqlx::query(
                    "INSERT OR IGNORE INTO tenant (id, share_key, needs_password, created_at) \
                     VALUES (?, ?, 1, ?)",
                )
                .bind(&id)
                .bind(key)
                .bind(now())
                .execute(&st.db)
                .await?;
                return Err(AppError::PasswordRequired);
            }
            Err(e) => return Err(e),
        },
        Some(t) => {
            if t.needs_password && t.share_token.is_empty() {
                return Err(AppError::PasswordRequired);
            }
            let stale = t
                .synced_at
                .map(|s| now() - s > st.cfg.sync_ttl_secs)
                .unwrap_or(true);
            if stale {
                if let Some(album) = t.immich_album.clone() {
                    // Thundering-herd guard: only one background resync per tenant.
                    let claimed = {
                        let mut g = st.syncing.lock().unwrap();
                        g.insert(id.clone())
                    };
                    if claimed {
                        let db = st.db.clone();
                        let immich = st.immich.clone();
                        let key = key.to_string();
                        let id2 = id.clone();
                        let token = (!t.share_token.is_empty()).then(|| t.share_token.clone());
                        let syncing = st.syncing.clone();
                        tokio::spawn(async move {
                            let res =
                                sync_assets(&db, &immich, &id2, &album, &key, token.as_deref())
                                    .await;
                            // Release the claim on both success and failure.
                            syncing.lock().unwrap().remove(&id2);
                            if let Err(e) = res {
                                tracing::warn!("background resync failed for {id2}: {e}");
                            }
                        });
                    }
                }
            }
        }
    }

    let tenant: Tenant = sqlx::query_as(
        "SELECT id, title, allow_marks, allow_notes, allow_download, public_write, \
         needs_password, share_token FROM tenant WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&st.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(tenant)
}

/// Verify a password against Immich, store the resulting token, provision + sync.
/// Returns the tenant id (the route uses it to set the visitor's unlock cookie).
/// `Forbidden` means the password was wrong.
pub async fn unlock(st: &AppState, key: &str, password: &str) -> AppResult<String> {
    let id = tenant_id(key);
    let token = st.immich.login(key, password).await?; // Forbidden on wrong password
    let info = st.immich.share_info(key, Some(&token)).await?;

    sqlx::query(
        "INSERT OR IGNORE INTO tenant (id, share_key, needs_password, created_at) \
         VALUES (?, ?, 1, ?)",
    )
    .bind(&id)
    .bind(key)
    .bind(now())
    .execute(&st.db)
    .await?;
    sqlx::query(
        "UPDATE tenant SET needs_password = 1, share_token = ?, immich_album = ?, \
         title = COALESCE(title, ?), allow_download = ? WHERE id = ?",
    )
    .bind(&token)
    .bind(&info.album_id)
    .bind(&info.title)
    .bind(info.allow_download)
    .bind(&id)
    .execute(&st.db)
    .await?;

    sync_assets(&st.db, &st.immich, &id, &info.album_id, key, Some(&token)).await?;
    Ok(id)
}

/// Re-fetch the album's asset list from Immich (read-only) and replace the cache.
pub async fn sync_assets(
    db: &SqlitePool,
    immich: &crate::immich::ImmichClient,
    tenant_id: &str,
    album_id: &str,
    key: &str,
    token: Option<&str>,
) -> AppResult<()> {
    let assets = immich.album_assets(album_id, key, token).await?;
    tracing::info!("syncing {} assets for tenant {}", assets.len(), tenant_id);

    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM asset WHERE tenant_id = ?")
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;

    for a in &assets {
        sqlx::query(
            "INSERT INTO asset \
             (tenant_id, asset_id, kind, width, height, taken_at, filename, immich_tags, exif_json) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(tenant_id)
        .bind(&a.id)
        .bind(&a.kind)
        .bind(a.width)
        .bind(a.height)
        .bind(a.taken_at)
        .bind(&a.filename)
        .bind(&a.tags)
        .bind(&a.exif)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("UPDATE tenant SET synced_at = ? WHERE id = ?")
        .bind(now())
        .bind(tenant_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

/// Verify an asset belongs to a tenant (defense-in-depth; Immich also gates by key).
pub async fn asset_belongs(db: &SqlitePool, tenant_id: &str, asset_id: &str) -> AppResult<bool> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM asset WHERE tenant_id = ? AND asset_id = ?")
            .bind(tenant_id)
            .bind(asset_id)
            .fetch_optional(db)
            .await?;
    Ok(row.is_some())
}

/// Look up an asset's filename (for download naming).
pub async fn asset_filename(db: &SqlitePool, tenant_id: &str, asset_id: &str) -> AppResult<String> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT filename FROM asset WHERE tenant_id = ? AND asset_id = ?")
            .bind(tenant_id)
            .bind(asset_id)
            .fetch_optional(db)
            .await?;
    Ok(row.map(|r| r.0).unwrap_or_else(|| format!("{asset_id}.bin")))
}
