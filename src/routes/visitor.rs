use axum::extract::State;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::state::{AppState, Visitor};
use crate::visitor as vis;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Me {
    id: String,
    name: String,
}

/// GET /api/visitor/me
pub async fn me(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
) -> AppResult<Json<Me>> {
    let name = vis::display_name(&st.db, &v.id).await;
    Ok(Json(Me { id: v.id, name }))
}

#[derive(Deserialize)]
pub struct NameIn {
    name: String,
}

#[derive(Serialize)]
pub struct NameOut {
    name: String,
}

/// POST /api/visitor/name  { name }
pub async fn set_name(
    State(st): State<AppState>,
    Extension(v): Extension<Visitor>,
    Json(input): Json<NameIn>,
) -> AppResult<Json<NameOut>> {
    // Sanitize: strip control chars, collapse whitespace, cap length.
    let name: String = input
        .name
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(40)
        .collect();

    sqlx::query("UPDATE visitor SET display_name = ? WHERE id = ?")
        .bind(&name)
        .bind(&v.id)
        .execute(&st.db)
        .await?;

    Ok(Json(NameOut { name }))
}
