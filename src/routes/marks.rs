use axum::extract::{Path, State};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::state::{now, AppState, Visitor};
use crate::{tenant, visitor};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkResult {
    marked: bool,
    mark_count: i64,
}

#[derive(Deserialize)]
pub struct BulkMarkIn {
    ids: Vec<String>,
    /// true (default) = mark; false = canonical unmark (clear for everyone).
    #[serde(default)]
    marked: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BulkItem {
    id: String,
    mark_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkMarkResult {
    items: Vec<BulkItem>,
}

/// POST /api/s/:key/mark  { ids: [...] } — marks all (idempotent, sets marked=true).
pub async fn bulk(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Path(key): Path<String>,
    Json(input): Json<BulkMarkIn>,
) -> AppResult<Json<BulkMarkResult>> {
    let t = tenant::ensure(&st, &key).await?;
    tenant::gate(&t, &v)?;
    if !t.allow_marks || !t.public_write {
        return Err(AppError::Forbidden);
    }
    if visitor::is_banned(&st.db, &v.id).await {
        return Err(AppError::Forbidden);
    }
    if !st.limiter.check(&v.id) {
        return Err(AppError::RateLimited);
    }
    if input.ids.is_empty() {
        return Err(AppError::BadRequest("no ids".into()));
    }
    if input.ids.len() > st.cfg.max_download_count {
        return Err(AppError::PayloadTooLarge);
    }

    let do_mark = input.marked.unwrap_or(true);
    let mut items = Vec::with_capacity(input.ids.len());
    for id in &input.ids {
        if !tenant::asset_belongs(&st.db, &t.id, id).await? {
            continue;
        }
        if do_mark {
            sqlx::query(
                "INSERT OR IGNORE INTO mark (tenant_id, asset_id, visitor_id, created_at) \
                 VALUES (?, ?, ?, ?)",
            )
            .bind(&t.id)
            .bind(id)
            .bind(&v.id)
            .bind(now())
            .execute(&st.db)
            .await?;
        } else {
            // Canonical unmark: clear all marks on this asset.
            sqlx::query("DELETE FROM mark WHERE tenant_id = ? AND asset_id = ?")
                .bind(&t.id)
                .bind(id)
                .execute(&st.db)
                .await?;
        }

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM mark WHERE tenant_id = ? AND asset_id = ?")
                .bind(&t.id)
                .bind(id)
                .fetch_one(&st.db)
                .await?;
        items.push(BulkItem {
            id: id.clone(),
            mark_count: count.0,
        });
    }

    Ok(Json(BulkMarkResult { items }))
}

/// POST /api/s/:key/asset/:id/mark  — toggles this visitor's mark.
pub async fn toggle(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Path((key, id)): Path<(String, String)>,
) -> AppResult<Json<MarkResult>> {
    let t = tenant::ensure(&st, &key).await?;
    tenant::gate(&t, &v)?;
    if !t.allow_marks || !t.public_write {
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

    // Canonical/shared mark: marked = ANY visitor has marked this asset.
    let exists: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM mark WHERE tenant_id = ? AND asset_id = ? LIMIT 1")
            .bind(&t.id)
            .bind(&id)
            .fetch_optional(&st.db)
            .await?;

    let marked = if exists.is_some() {
        // Unmark is canonical: clear it for everyone, regardless of who marked it.
        sqlx::query("DELETE FROM mark WHERE tenant_id = ? AND asset_id = ?")
            .bind(&t.id)
            .bind(&id)
            .execute(&st.db)
            .await?;
        false
    } else {
        sqlx::query(
            "INSERT INTO mark (tenant_id, asset_id, visitor_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&t.id)
        .bind(&id)
        .bind(&v.id)
        .bind(now())
        .execute(&st.db)
        .await?;
        true
    };

    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM mark WHERE tenant_id = ? AND asset_id = ?")
            .bind(&t.id)
            .bind(&id)
            .fetch_one(&st.db)
            .await?;

    Ok(Json(MarkResult {
        marked,
        mark_count: count.0,
    }))
}
