use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use crate::state::AppState;

/// Serve the SPA shell for /s/:key. The built frontend uses absolute /assets/
/// references (vite base '/'), so the same HTML works under any share path.
pub async fn spa(State(st): State<AppState>) -> Response {
    let path = format!("{}/index.html", st.cfg.web_dir);
    match tokio::fs::read_to_string(&path).await {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!("failed to read {path}: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "frontend not built").into_response()
        }
    }
}

/// Minimal landing page at /.
pub async fn root() -> Response {
    Html("<!doctype html><meta charset=utf-8><title>Interactive Immich Public Proxy</title><body style=\"font-family:system-ui;background:#111;color:#ddd;padding:2rem\"><h1>Interactive Immich Public Proxy</h1><p>Open a share link to view an album.</p></body>")
        .into_response()
}
