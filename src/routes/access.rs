use axum::extract::{Path, State};
use axum::http::header::SET_COOKIE;
use axum::http::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::error::AppResult;
use crate::state::AppState;
use crate::{tenant, visitor};

#[derive(Deserialize)]
pub struct UnlockIn {
    password: String,
}

/// POST /api/s/:key/unlock  { password }
/// Verifies the password against Immich, then sets a signed unlock cookie so this
/// visitor can view the password-protected album.
pub async fn unlock(
    State(st): State<AppState>,
    Path(key): Path<String>,
    Json(input): Json<UnlockIn>,
) -> AppResult<Response> {
    let id = tenant::unlock(&st, &key, &input.password).await?; // Forbidden on wrong password
    let cookie = visitor::unlock_set_cookie(&st.cfg.cookie_secret, &id);
    let mut resp = Json(json!({ "ok": true })).into_response();
    if let Ok(hv) = HeaderValue::from_str(&cookie) {
        resp.headers_mut().append(SET_COOKIE, hv);
    }
    Ok(resp)
}
