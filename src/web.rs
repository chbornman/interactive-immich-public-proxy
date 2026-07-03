use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

use crate::state::AppState;

/// Serve the SPA shell for / and /s/:key. The built frontend uses absolute
/// /assets/ references (vite base '/'), so the same HTML works under any path.
/// index.html is read once at startup (see main.rs) and served from memory.
pub async fn spa(State(st): State<AppState>) -> Response {
    if st.index_html.is_empty() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "frontend not built").into_response();
    }
    Html(st.index_html.clone()).into_response()
}
