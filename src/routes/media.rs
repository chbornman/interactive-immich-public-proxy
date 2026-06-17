use axum::body::Body;
use axum::extract::{Path, State};
use axum::Extension;
use axum::http::header::{
    ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG, LAST_MODIFIED,
    RANGE,
};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;

use crate::error::{AppError, AppResult};
use crate::state::{AppState, Visitor};
use crate::tenant;

/// GET /api/s/:key/asset/:id/:size  (size = thumbnail | preview | original)
/// Streams bytes from Immich read-only, relaying Range/Content-Range for video seeking.
pub async fn get(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Path((key, id, size)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if !matches!(size.as_str(), "thumbnail" | "preview" | "original") {
        return Err(AppError::BadRequest("invalid size".into()));
    }

    let t = tenant::ensure(&st, &key).await?;
    tenant::gate(&t, &v)?;
    if !tenant::asset_belongs(&st.db, &t.id, &id).await? {
        return Err(AppError::NotFound);
    }

    let range = headers.get(RANGE).and_then(|v| v.to_str().ok());
    let upstream = st.immich.fetch_bytes(&id, &size, &key, t.token(), range).await?;

    let status = StatusCode::from_u16(upstream.status().as_u16())
        .unwrap_or(StatusCode::OK);

    let cache = if size == "original" {
        "public, max-age=86400"
    } else {
        "public, max-age=2592000, immutable"
    };

    let mut builder = Response::builder().status(status).header(CACHE_CONTROL, cache);

    // Relay the headers that matter for media + range.
    for name in [
        CONTENT_TYPE,
        CONTENT_LENGTH,
        CONTENT_RANGE,
        ACCEPT_RANGES,
        ETAG,
        LAST_MODIFIED,
    ] {
        if let Some(v) = upstream.headers().get(&name) {
            builder = builder.header(name, v.clone());
        }
    }

    let body = Body::from_stream(upstream.bytes_stream());
    builder
        .body(body)
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))
}
