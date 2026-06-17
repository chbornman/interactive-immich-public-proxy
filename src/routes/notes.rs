use axum::extract::{Path, State};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::state::{now, AppState, Visitor};
use crate::{tenant, visitor};

#[derive(Deserialize)]
pub struct NoteIn {
    body: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteOut {
    id: i64,
    name: String,
    body: String,
    created_at: i64,
}

/// POST /api/s/:key/asset/:id/note  — add a note (plain text, escaped on render).
pub async fn add(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Path((key, id)): Path<(String, String)>,
    Json(input): Json<NoteIn>,
) -> AppResult<Json<NoteOut>> {
    let t = tenant::ensure(&st, &key).await?;
    tenant::gate(&t, &v)?;
    if !t.allow_notes || !t.public_write {
        return Err(AppError::Forbidden);
    }
    if visitor::is_banned(&st.db, &v.id).await {
        return Err(AppError::Forbidden);
    }
    if !st.limiter.check(&v.id) {
        return Err(AppError::RateLimited);
    }
    if !tenant::asset_belongs(&st.db, &t.id, &id).await? {
        return Err(AppError::NotFound);
    }

    let body = input.body.trim();
    if body.is_empty() {
        return Err(AppError::BadRequest("empty note".into()));
    }
    let body: String = body.chars().take(st.cfg.max_note_len).collect();

    let name = visitor::display_name(&st.db, &v.id).await;
    let ts = now();

    let res = sqlx::query(
        "INSERT INTO note (tenant_id, asset_id, visitor_id, display_name, body, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&t.id)
    .bind(&id)
    .bind(&v.id)
    .bind(&name)
    .bind(&body)
    .bind(ts)
    .execute(&st.db)
    .await?;

    Ok(Json(NoteOut {
        id: res.last_insert_rowid(),
        name,
        body,
        created_at: ts,
    }))
}
