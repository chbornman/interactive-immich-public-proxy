use axum::extract::State;
use axum::Json;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AlbumSummary {
    /// The share key IS the public link for a listed album, so exposing it here
    /// is intentional.
    pub(crate) key: String,
    pub(crate) title: Option<String>,
    pub(crate) photos: i64,
    pub(crate) videos: i64,
    /// Most recent asset, used as the card thumbnail (None for empty albums).
    pub(crate) cover: Option<String>,
}

/// Albums eligible for the public index: listed, not password-protected, and
/// actually provisioned (immich_album set). Password-protected albums must
/// never appear here — not even their titles.
pub(crate) async fn listed_albums(db: &SqlitePool) -> Result<Vec<AlbumSummary>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.share_key AS key, t.title, \
         (SELECT COUNT(*) FROM asset a WHERE a.tenant_id = t.id AND a.kind != 'VIDEO') AS photos, \
         (SELECT COUNT(*) FROM asset a WHERE a.tenant_id = t.id AND a.kind = 'VIDEO') AS videos, \
         (SELECT a.asset_id FROM asset a WHERE a.tenant_id = t.id \
          ORDER BY a.taken_at DESC, a.asset_id DESC LIMIT 1) AS cover \
         FROM tenant t \
         WHERE t.listed = 1 AND t.needs_password = 0 AND t.immich_album IS NOT NULL \
         ORDER BY t.created_at DESC",
    )
    .fetch_all(db)
    .await
}

/// GET /api/albums — public index of listed albums.
pub async fn index(State(st): State<AppState>) -> AppResult<Json<Vec<AlbumSummary>>> {
    Ok(Json(listed_albums(&st.db).await?))
}
