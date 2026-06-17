use axum::extract::{Path, Query, State};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use sqlx::QueryBuilder;

use crate::error::{AppError, AppResult};
use crate::state::{AppState, Visitor};
use crate::tenant;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInfo {
    title: String,
    total: i64,
    photos: i64,
    videos: i64,
}

/// GET /api/s/:key/album
pub async fn album(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Path(key): Path<String>,
) -> AppResult<Json<AlbumInfo>> {
    let t = tenant::ensure(&st, &key).await?;
    tenant::gate(&t, &v)?;
    let row: (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(kind = 'VIDEO'), 0) FROM asset WHERE tenant_id = ?",
    )
    .bind(&t.id)
    .fetch_one(&st.db)
    .await?;
    let (total, videos) = row;
    Ok(Json(AlbumInfo {
        title: t.title.unwrap_or_default(),
        total,
        photos: total - videos,
        videos,
    }))
}

#[derive(Deserialize)]
pub struct ListParams {
    #[serde(default)]
    cursor: String,
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    filter: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    q: String,
}

#[derive(sqlx::FromRow)]
struct AssetRow {
    asset_id: String,
    kind: String,
    width: i64,
    height: i64,
    taken_at: i64,
    filename: String,
    mark_count: i64,
    has_note: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AssetOut {
    id: String,
    kind: String,
    width: i64,
    height: i64,
    taken_at: i64,
    filename: String,
    mark_count: i64,
    has_note: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsPage {
    items: Vec<AssetOut>,
    next_cursor: Option<String>,
}

/// GET /api/s/:key/assets?cursor=&limit=100&filter=all|marked|noted&q=
pub async fn list(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Path(key): Path<String>,
    Query(p): Query<ListParams>,
) -> AppResult<Json<AssetsPage>> {
    let t = tenant::ensure(&st, &key).await?;
    tenant::gate(&t, &v)?;
    let limit = p.limit.unwrap_or(100).clamp(1, 200);

    let mut qb: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new(
        "SELECT a.asset_id, a.kind, a.width, a.height, a.taken_at, a.filename, \
         (SELECT COUNT(*) FROM mark m WHERE m.tenant_id = a.tenant_id AND m.asset_id = a.asset_id) AS mark_count, \
         EXISTS(SELECT 1 FROM note n WHERE n.tenant_id = a.tenant_id AND n.asset_id = a.asset_id AND n.hidden = 0) AS has_note \
         FROM asset a WHERE a.tenant_id = ",
    );
    qb.push_bind(t.id.clone());

    // Cursor: "<taken_at>:<asset_id>", DESC order => fetch strictly "older".
    if let Some((cts, cid)) = p.cursor.split_once(':') {
        if let Ok(cts) = cts.parse::<i64>() {
            qb.push(" AND (a.taken_at < ");
            qb.push_bind(cts);
            qb.push(" OR (a.taken_at = ");
            qb.push_bind(cts);
            qb.push(" AND a.asset_id < ");
            qb.push_bind(cid.to_string());
            qb.push("))");
        }
    }

    match p.filter.as_str() {
        "marked" => {
            qb.push(
                " AND EXISTS(SELECT 1 FROM mark m WHERE m.tenant_id = a.tenant_id AND m.asset_id = a.asset_id)",
            );
        }
        "noted" => {
            qb.push(
                " AND EXISTS(SELECT 1 FROM note n WHERE n.tenant_id = a.tenant_id AND n.asset_id = a.asset_id AND n.hidden = 0)",
            );
        }
        _ => {}
    }

    // Independent type dimension (combines with the filter above).
    match p.kind.as_str() {
        "image" => {
            qb.push(" AND a.kind = 'IMAGE'");
        }
        "video" => {
            qb.push(" AND a.kind = 'VIDEO'");
        }
        _ => {}
    }

    let q = p.q.trim();
    if !q.is_empty() {
        let like = format!("%{}%", q.replace('%', "\\%").replace('_', "\\_"));
        qb.push(" AND (a.filename LIKE ");
        qb.push_bind(like.clone());
        qb.push(" ESCAPE '\\' OR a.immich_tags LIKE ");
        qb.push_bind(like.clone());
        qb.push(" ESCAPE '\\' OR EXISTS(SELECT 1 FROM note n WHERE n.tenant_id = a.tenant_id AND n.asset_id = a.asset_id AND n.hidden = 0 AND n.body LIKE ");
        qb.push_bind(like);
        qb.push(" ESCAPE '\\'))");
    }

    qb.push(" ORDER BY a.taken_at DESC, a.asset_id DESC LIMIT ");
    qb.push_bind(limit + 1);

    let mut rows: Vec<AssetRow> = qb.build_query_as().fetch_all(&st.db).await?;

    let next_cursor = if rows.len() as i64 > limit {
        let last = &rows[limit as usize - 1];
        Some(format!("{}:{}", last.taken_at, last.asset_id))
    } else {
        None
    };
    rows.truncate(limit as usize);

    let items = rows
        .into_iter()
        .map(|r| AssetOut {
            id: r.asset_id,
            kind: r.kind,
            width: r.width,
            height: r.height,
            taken_at: r.taken_at,
            filename: r.filename,
            mark_count: r.mark_count,
            has_note: r.has_note,
        })
        .collect();

    Ok(Json(AssetsPage { items, next_cursor }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NoteOut {
    id: i64,
    name: String,
    body: String,
    created_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMeta {
    mark_count: i64,
    marked: bool,
    notes: Vec<NoteOut>,
    exif: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct NoteRow {
    id: i64,
    display_name: String,
    body: String,
    created_at: i64,
}

/// GET /api/s/:key/asset/:id/meta
pub async fn meta(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Path((key, id)): Path<(String, String)>,
) -> AppResult<Json<AssetMeta>> {
    let t = tenant::ensure(&st, &key).await?;
    tenant::gate(&t, &v)?;
    if !tenant::asset_belongs(&st.db, &t.id, &id).await? {
        return Err(AppError::NotFound);
    }

    let mark_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM mark WHERE tenant_id = ? AND asset_id = ?")
            .bind(&t.id)
            .bind(&id)
            .fetch_one(&st.db)
            .await?;

    // Canonical/shared: marked = ANY visitor has marked it.
    let marked: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM mark WHERE tenant_id = ? AND asset_id = ? LIMIT 1")
            .bind(&t.id)
            .bind(&id)
            .fetch_optional(&st.db)
            .await?;

    let notes: Vec<NoteRow> = sqlx::query_as(
        "SELECT id, display_name, body, created_at FROM note \
         WHERE tenant_id = ? AND asset_id = ? AND hidden = 0 ORDER BY created_at ASC",
    )
    .bind(&t.id)
    .bind(&id)
    .fetch_all(&st.db)
    .await?;

    let exif_row: Option<(String,)> =
        sqlx::query_as("SELECT exif_json FROM asset WHERE tenant_id = ? AND asset_id = ?")
            .bind(&t.id)
            .bind(&id)
            .fetch_optional(&st.db)
            .await?;
    let exif: serde_json::Value = exif_row
        .and_then(|r| serde_json::from_str(&r.0).ok())
        .unwrap_or(serde_json::Value::Null);

    Ok(Json(AssetMeta {
        mark_count: mark_count.0,
        marked: marked.is_some(),
        notes: notes
            .into_iter()
            .map(|n| NoteOut {
                id: n.id,
                name: n.display_name,
                body: n.body,
                created_at: n.created_at,
            })
            .collect(),
        exif,
    }))
}
