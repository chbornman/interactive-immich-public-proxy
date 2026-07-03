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
    /// Password-protected shares are listed (title only) with a lock indicator;
    /// their contents stay gated behind the Immich password.
    pub(crate) needs_password: bool,
}

/// Albums eligible for the public index: listed and actually provisioned
/// (immich_album set). Password-protected albums appear too, flagged so the
/// index can show a lock — hiding one entirely is the admin `listed` toggle.
pub(crate) async fn listed_albums(db: &SqlitePool) -> Result<Vec<AlbumSummary>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.share_key AS key, t.title, \
         (SELECT COUNT(*) FROM asset a WHERE a.tenant_id = t.id AND a.kind != 'VIDEO') AS photos, \
         (SELECT COUNT(*) FROM asset a WHERE a.tenant_id = t.id AND a.kind = 'VIDEO') AS videos, \
         t.needs_password \
         FROM tenant t \
         WHERE t.listed = 1 AND t.immich_album IS NOT NULL \
         ORDER BY t.created_at DESC",
    )
    .fetch_all(db)
    .await
}

/// GET /api/albums — public index of listed albums.
pub async fn index(State(st): State<AppState>) -> AppResult<Json<Vec<AlbumSummary>>> {
    Ok(Json(listed_albums(&st.db).await?))
}
