use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::tenant;

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TenantSummary {
    id: String,
    title: Option<String>,
    allow_marks: bool,
    allow_notes: bool,
    allow_download: bool,
    public_write: bool,
    synced_at: Option<i64>,
    assets: i64,
    marks: i64,
    notes: i64,
}

/// GET /admin/tenants
pub async fn tenants(State(st): State<AppState>) -> AppResult<Json<Vec<TenantSummary>>> {
    let rows: Vec<TenantSummary> = sqlx::query_as(
        "SELECT t.id, t.title, t.allow_marks, t.allow_notes, t.allow_download, t.public_write, t.synced_at, \
         (SELECT COUNT(*) FROM asset a WHERE a.tenant_id = t.id) AS assets, \
         (SELECT COUNT(*) FROM mark m WHERE m.tenant_id = t.id) AS marks, \
         (SELECT COUNT(*) FROM note n WHERE n.tenant_id = t.id AND n.hidden = 0) AS notes \
         FROM tenant t ORDER BY t.created_at DESC",
    )
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TenantPatch {
    title: Option<String>,
    allow_marks: Option<bool>,
    allow_notes: Option<bool>,
    allow_download: Option<bool>,
    public_write: Option<bool>,
}

/// PATCH /admin/tenant/:id
pub async fn patch_tenant(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(p): Json<TenantPatch>,
) -> AppResult<Json<serde_json::Value>> {
    macro_rules! set_bool {
        ($field:literal, $val:expr) => {
            if let Some(v) = $val {
                sqlx::query(concat!("UPDATE tenant SET ", $field, " = ? WHERE id = ?"))
                    .bind(v as i64)
                    .bind(&id)
                    .execute(&st.db)
                    .await?;
            }
        };
    }
    set_bool!("allow_marks", p.allow_marks);
    set_bool!("allow_notes", p.allow_notes);
    set_bool!("allow_download", p.allow_download);
    set_bool!("public_write", p.public_write);
    if let Some(title) = p.title {
        sqlx::query("UPDATE tenant SET title = ? WHERE id = ?")
            .bind(title)
            .bind(&id)
            .execute(&st.db)
            .await?;
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/tenant/:id/resync
pub async fn resync(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let row: Option<(String, Option<String>, String)> =
        sqlx::query_as("SELECT share_key, immich_album, share_token FROM tenant WHERE id = ?")
            .bind(&id)
            .fetch_optional(&st.db)
            .await?;
    let (key, album, token) = row.ok_or(AppError::NotFound)?;
    let album = album.ok_or(AppError::NotFound)?;
    let token = (!token.is_empty()).then_some(token);
    tenant::sync_assets(&st.db, &st.immich, &id, &album, &key, token.as_deref()).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NotesQuery {
    tenant: String,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdminNote {
    id: i64,
    asset_id: String,
    visitor_id: String,
    display_name: String,
    body: String,
    created_at: i64,
    hidden: bool,
}

/// GET /admin/notes?tenant=ID  (includes hidden)
pub async fn notes(
    State(st): State<AppState>,
    Query(q): Query<NotesQuery>,
) -> AppResult<Json<Vec<AdminNote>>> {
    let rows: Vec<AdminNote> = sqlx::query_as(
        "SELECT id, asset_id, visitor_id, display_name, body, created_at, hidden \
         FROM note WHERE tenant_id = ? ORDER BY created_at DESC LIMIT 1000",
    )
    .bind(&q.tenant)
    .fetch_all(&st.db)
    .await?;
    Ok(Json(rows))
}

/// POST /admin/note/:id/hide  — toggles a note's hidden flag.
pub async fn hide_note(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE note SET hidden = 1 - hidden WHERE id = ?")
        .bind(id)
        .execute(&st.db)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /admin/visitor/:id/ban  — bans a visitor and hides their notes.
pub async fn ban_visitor(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("UPDATE visitor SET banned = 1 WHERE id = ?")
        .bind(&id)
        .execute(&st.db)
        .await?;
    sqlx::query("UPDATE note SET hidden = 1 WHERE visitor_id = ?")
        .bind(&id)
        .execute(&st.db)
        .await?;
    sqlx::query("DELETE FROM mark WHERE visitor_id = ?")
        .bind(&id)
        .execute(&st.db)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
