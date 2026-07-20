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
                            match res {
                                Ok(()) => {
                                    // Sync works again: drop any stale dead marker.
                                    // Relisting stays a human decision.
                                    if let Err(e) = clear_tenant_dead(&db, &id2).await {
                                        tracing::warn!("clearing dead flag for {id2}: {e}");
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("background resync failed for {id2}: {e}");
                                    // Probe the share to tell a revoked key apart from a
                                    // transient failure (share_info classifies the 401 body).
                                    match immich.share_info(&key, token.as_deref()).await {
                                        Err(AppError::NotFound) => {
                                            // Key revoked: delist so we stop advertising
                                            // an album we can no longer serve.
                                            match mark_tenant_dead(&db, &id2, now()).await {
                                                Ok(true) => tracing::warn!(
                                                    "share key revoked; tenant delisted: {id2}"
                                                ),
                                                Ok(false) => {}
                                                Err(e) => tracing::warn!(
                                                    "marking tenant {id2} dead: {e}"
                                                ),
                                            }
                                        }
                                        Err(AppError::PasswordRequired) => {
                                            // Alive but (re)locked: gate visitors behind
                                            // the password prompt instead of broken media.
                                            if let Err(e) =
                                                mark_tenant_needs_password(&db, &id2).await
                                            {
                                                tracing::warn!(
                                                    "marking tenant {id2} locked: {e}"
                                                );
                                            }
                                        }
                                        // Probe succeeded (share alive) or itself failed
                                        // (network): treat the sync failure as transient.
                                        _ => {}
                                    }
                                }
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

/// How many per-asset metadata fetches to keep in flight during a sync.
/// Immich is on the internal network, but a first sync of a large album issues
/// one request per asset — this bounds the burst.
const DETAIL_CONCURRENCY: usize = 8;

/// Re-sync the album's asset list from Immich (read-only), incrementally.
///
/// Immich v3's timeline API returns ids only, so full metadata costs one request
/// per asset. To keep re-syncs cheap we diff against the cache: assets already
/// stored are left untouched, assets no longer in the album are pruned, and only
/// genuinely new ids are fetched. A re-sync of an unchanged album issues no
/// per-asset requests at all.
pub async fn sync_assets(
    db: &SqlitePool,
    immich: &crate::immich::ImmichClient,
    tenant_id: &str,
    album_id: &str,
    key: &str,
    token: Option<&str>,
) -> AppResult<()> {
    let remote: Vec<String> = immich.album_asset_ids(album_id, key, token).await?;
    let remote_set: std::collections::HashSet<&str> =
        remote.iter().map(|s| s.as_str()).collect();

    let cached: Vec<(String,)> = sqlx::query_as("SELECT asset_id FROM asset WHERE tenant_id = ?")
        .bind(tenant_id)
        .fetch_all(db)
        .await?;
    let cached_set: std::collections::HashSet<String> =
        cached.into_iter().map(|r| r.0).collect();

    let stale: Vec<&String> = cached_set
        .iter()
        .filter(|id| !remote_set.contains(id.as_str()))
        .collect();
    let new_ids: Vec<&String> = remote.iter().filter(|id| !cached_set.contains(*id)).collect();

    tracing::info!(
        "syncing tenant {}: {} in album, {} cached, {} new, {} pruned",
        tenant_id,
        remote.len(),
        cached_set.len(),
        new_ids.len(),
        stale.len()
    );

    // An album that legitimately has no assets is possible, but a sudden drop to
    // zero against a non-empty cache is the signature of an upstream API change
    // (v3 did exactly this). Refuse to wipe a populated cache on that signal.
    if remote.is_empty() && !cached_set.is_empty() {
        return Err(AppError::Upstream(format!(
            "album {album_id} returned 0 assets but {} are cached; refusing to wipe",
            cached_set.len()
        )));
    }

    // Fetch metadata for new ids only, bounded concurrency.
    let mut fetched: Vec<crate::immich::Asset> = Vec::with_capacity(new_ids.len());
    for chunk in new_ids.chunks(DETAIL_CONCURRENCY) {
        let mut set = tokio::task::JoinSet::new();
        for id in chunk {
            let (immich, id, key) = (immich.clone(), (*id).clone(), key.to_string());
            let token = token.map(|t| t.to_string());
            set.spawn(async move {
                immich.asset_detail(&id, &key, token.as_deref()).await
            });
        }
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(Ok(a)) => fetched.push(a),
                // One bad asset shouldn't abort the whole sync; it'll be retried
                // on the next pass since it stays absent from the cache.
                Ok(Err(e)) => tracing::warn!("asset detail failed during sync: {e}"),
                Err(e) => tracing::warn!("asset detail task panicked: {e}"),
            }
        }
    }

    let mut tx = db.begin().await?;

    for id in &stale {
        sqlx::query("DELETE FROM asset WHERE tenant_id = ? AND asset_id = ?")
            .bind(tenant_id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }

    for a in &fetched {
        sqlx::query(
            "INSERT OR REPLACE INTO asset \
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

/// Mark a tenant's share key as revoked: delist it and stamp dead_since,
/// keeping the first detection time across repeated failures. Returns true
/// only on the first detection (so the caller can log the transition once).
pub(crate) async fn mark_tenant_dead(db: &SqlitePool, id: &str, now: i64) -> AppResult<bool> {
    let was_dead: Option<(Option<i64>,)> =
        sqlx::query_as("SELECT dead_since FROM tenant WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
    sqlx::query("UPDATE tenant SET listed = 0, dead_since = COALESCE(dead_since, ?) WHERE id = ?")
        .bind(now)
        .bind(id)
        .execute(db)
        .await?;
    Ok(matches!(was_dead, Some((None,))))
}

/// The share is alive but (re)locked: gate visitors behind the password prompt.
/// The stale token is cleared too — Immich already rejected it, and an empty
/// share_token is what makes `ensure` return PasswordRequired so previously
/// unlocked visitors re-prompt instead of fetching media with a dead token.
pub(crate) async fn mark_tenant_needs_password(db: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE tenant SET needs_password = 1, share_token = '' WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// A successful resync proves the share key works again: forget dead_since.
/// Deliberately does NOT restore `listed` — relisting is an admin decision.
pub(crate) async fn clear_tenant_dead(db: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("UPDATE tenant SET dead_since = NULL WHERE id = ? AND dead_since IS NOT NULL")
        .bind(id)
        .execute(db)
        .await?;
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
