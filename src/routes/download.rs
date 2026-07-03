use std::io::Write;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::Response;
use axum::{Extension, Json};
use serde::Deserialize;

use crate::error::{AppError, AppResult};
use crate::state::{AppState, Visitor};
use crate::{tenant, visitor};

#[derive(Deserialize)]
pub struct DownloadIn {
    ids: Vec<String>,
}

/// Strip path separators / quotes for safe Content-Disposition + zip entry names.
pub(crate) fn safe_name(name: &str) -> String {
    let n: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | '"' | '\n' | '\r') { '_' } else { c })
        .collect();
    if n.trim().is_empty() {
        "file".to_string()
    } else {
        n
    }
}

/// POST /api/s/:key/download  { ids: [...] }
/// One id → original passthrough; many → an in-memory (buffered) ZIP of originals.
pub async fn post(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Path(key): Path<String>,
    Json(input): Json<DownloadIn>,
) -> AppResult<Response> {
    let t = tenant::ensure(&st, &key).await?;
    tenant::gate(&t, &v)?;
    if !t.allow_download {
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

    // Validate membership for every requested id.
    for id in &input.ids {
        if !tenant::asset_belongs(&st.db, &t.id, id).await? {
            return Err(AppError::NotFound);
        }
    }

    // Single file: stream the original through with an attachment disposition.
    if input.ids.len() == 1 {
        let id = &input.ids[0];
        let filename = safe_name(&tenant::asset_filename(&st.db, &t.id, id).await?);
        let upstream = st.immich.fetch_bytes(id, "original", &key, t.token(), None).await?;

        let mut builder = Response::builder().status(StatusCode::OK).header(
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        );
        if let Some(ct) = upstream.headers().get(CONTENT_TYPE) {
            builder = builder.header(CONTENT_TYPE, ct.clone());
        }
        if let Some(cl) = upstream.headers().get(CONTENT_LENGTH) {
            builder = builder.header(CONTENT_LENGTH, cl.clone());
        }
        return builder
            .body(Body::from_stream(upstream.bytes_stream()))
            .map_err(|e| AppError::Other(anyhow::anyhow!(e)));
    }

    // Multiple: fetch originals into memory (bounded), then build an in-memory ZIP.
    let mut files = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut total: u64 = 0;

    for (i, id) in input.ids.iter().enumerate() {
        let resp = st.immich.fetch_bytes(id, "original", &key, t.token(), None).await?;
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| AppError::Upstream(e.to_string()))?;
        total += bytes.len() as u64;
        if total > st.cfg.max_download_bytes {
            return Err(AppError::PayloadTooLarge);
        }

        let mut name = safe_name(&tenant::asset_filename(&st.db, &t.id, id).await?);
        if !seen.insert(name.clone()) {
            name = format!("{i}_{name}");
            seen.insert(name.clone());
        }
        files.push((name, bytes));
    }

    let zip_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, zip::result::ZipError> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            for (name, data) in files {
                zip.start_file(name, opts)?;
                zip.write_all(&data)?;
            }
            zip.finish()?;
        }
        Ok(cursor.into_inner())
    })
    .await
    .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?
    .map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/zip")
        .header(CONTENT_DISPOSITION, "attachment; filename=\"album.zip\"")
        .header(CONTENT_LENGTH, zip_bytes.len())
        .body(Body::from(zip_bytes))
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))
}
