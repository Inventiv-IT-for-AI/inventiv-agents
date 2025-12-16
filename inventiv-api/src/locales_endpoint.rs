use axum::{response::IntoResponse, Json, extract::State};
use serde::Serialize;
use std::sync::Arc;

use crate::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct LocaleResponse {
    pub code: String,
    pub name: String,
    pub native_name: Option<String>,
    pub direction: String, // ltr|rtl
}

pub async fn list_locales(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rows: Vec<LocaleResponse> = sqlx::query_as(
        r#"
        SELECT code, name, native_name, direction
        FROM locales
        WHERE is_active = true
        ORDER BY code ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows).into_response()
}


