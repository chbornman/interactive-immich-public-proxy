pub mod access;
pub mod admin;
pub mod albums;
pub mod assets;
pub mod download;
pub mod marks;
pub mod media;
pub mod notes;
pub mod visitor;

use axum::middleware::from_fn_with_state;
use axum::routing::{get, patch, post};
use axum::Router;
use tower_http::compression::predicate::{DefaultPredicate, NotForContentType, Predicate};
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::state::AppState;
use crate::visitor::middleware as visitor_mw;
use crate::web;

pub fn router(st: AppState) -> Router {
    // Public API — every route gets a signed visitor cookie via middleware.
    let api = Router::new()
        .route("/visitor/me", get(visitor::me))
        .route("/visitor/name", post(visitor::set_name))
        .route("/albums", get(albums::index))
        .route("/s/{key}/unlock", post(access::unlock))
        .route("/s/{key}/album", get(assets::album))
        .route("/s/{key}/assets", get(assets::list))
        .route("/s/{key}/asset/{id}/meta", get(assets::meta))
        .route("/s/{key}/asset/{id}/mark", post(marks::toggle))
        .route("/s/{key}/mark", post(marks::bulk))
        .route("/s/{key}/asset/{id}/note", post(notes::add))
        .route("/s/{key}/media/{id}/{size}", get(media::get))
        .route("/s/{key}/download", post(download::post))
        .layer(from_fn_with_state(st.clone(), visitor_mw));

    // Admin API — gated by Authentik forward-auth at Caddy (no visitor cookie).
    let admin = Router::new()
        .route("/tenants", get(admin::tenants))
        .route("/tenant/{id}", patch(admin::patch_tenant).delete(admin::delete_tenant))
        .route("/tenant/{id}/resync", post(admin::resync))
        .route("/notes", get(admin::notes))
        .route("/note/{id}/hide", post(admin::hide_note))
        .route("/visitor/{id}/ban", post(admin::ban_visitor));

    // Compress text responses (JSON/HTML/JS/CSS) but skip media: gzipping video
    // or zip payloads breaks HTTP range/seek and just wastes CPU.
    let compression = CompressionLayer::new().compress_when(
        DefaultPredicate::new()
            .and(NotForContentType::const_new("video/"))
            .and(NotForContentType::const_new("application/zip"))
            .and(NotForContentType::const_new("application/octet-stream")),
    );

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        // "/" serves the SPA too: with no share key it renders the public album index.
        .route("/", get(web::spa))
        // Immich mints links as /share/<key>; /s/<key> is a short alias. Both serve the SPA.
        .route("/share/{key}", get(web::spa))
        .route("/s/{key}", get(web::spa))
        .nest("/api", api)
        .nest("/admin", admin)
        .fallback_service(ServeDir::new(&st.cfg.web_dir))
        .layer(compression)
        .layer(TraceLayer::new_for_http())
        .with_state(st)
}
