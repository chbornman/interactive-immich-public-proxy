use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Application error type that converts into a JSON HTTP response.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("password required")]
    PasswordRequired,
    #[error("too many requests")]
    RateLimited,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("upstream immich error: {0}")]
    Upstream(String),
    #[error(transparent)]
    Db(#[from] sqlx::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Distinct shape so the SPA can show a password prompt.
        if matches!(self, AppError::PasswordRequired) {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "password required", "passwordRequired": true })),
            )
                .into_response();
        }
        let (status, msg) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".to_string()),
            AppError::PasswordRequired => unreachable!(),
            AppError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "slow down".to_string()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "too large".to_string()),
            AppError::Upstream(m) => {
                tracing::warn!("upstream error: {m}");
                (StatusCode::BAD_GATEWAY, "upstream error".to_string())
            }
            AppError::Db(e) => {
                tracing::error!("db error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string())
            }
            AppError::Other(e) => {
                tracing::error!("error: {e:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error".to_string())
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
