use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct SettingDefinitionRow {
    pub key: String,
    pub scope: String,
    pub value_type: String,
    pub min_int: Option<i64>,
    pub max_int: Option<i64>,
    pub default_int: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct ProviderParamsRow {
    pub provider_id: Uuid,
    pub provider_name: String,
    pub provider_code: String,
    pub worker_instance_startup_timeout_s: Option<i64>,
    pub instance_startup_timeout_s: Option<i64>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateProviderParamsRequest {
    /// If null/missing, falls back to env/default (removes provider override).
    pub worker_instance_startup_timeout_s: Option<i64>,
    /// If null/missing, falls back to env/default (removes provider override).
    pub instance_startup_timeout_s: Option<i64>,
}

fn validate_timeout_s(v: i64) -> bool {
    // Keep sane bounds: 30s .. 24h
    v >= 30 && v <= 86_400
}

#[utoipa::path(
    get,
    path = "/settings/definitions",
    tag = "Settings",
    responses((status = 200, description = "Settings definitions catalog", body = Vec<SettingDefinitionRow>))
)]
pub async fn list_settings_definitions(State(state): State<Arc<AppState>>) -> Json<Vec<SettingDefinitionRow>> {
    let rows = sqlx::query_as::<_, SettingDefinitionRow>(
        r#"
        SELECT key, scope, value_type, min_int, max_int, default_int, description
        FROM settings_definitions
        ORDER BY key
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}

#[utoipa::path(
    get,
    path = "/providers/params",
    tag = "Settings",
    responses((status = 200, description = "Provider-scoped parameters", body = Vec<ProviderParamsRow>))
)]
pub async fn list_provider_params(State(state): State<Arc<AppState>>) -> Json<Vec<ProviderParamsRow>> {
    let rows = sqlx::query_as::<_, ProviderParamsRow>(
        r#"
        SELECT
          p.id as provider_id,
          p.name as provider_name,
          p.code as provider_code,
          MAX(CASE WHEN s.key = 'WORKER_INSTANCE_STARTUP_TIMEOUT_S' THEN s.value_int END) AS worker_instance_startup_timeout_s,
          MAX(CASE WHEN s.key = 'INSTANCE_STARTUP_TIMEOUT_S' THEN s.value_int END) AS instance_startup_timeout_s
        FROM providers p
        LEFT JOIN provider_settings s ON s.provider_id = p.id
        GROUP BY p.id, p.name, p.code
        ORDER BY p.name
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}

#[utoipa::path(
    put,
    path = "/providers/{id}/params",
    tag = "Settings",
    request_body = UpdateProviderParamsRequest,
    responses(
        (status = 200, description = "Updated"),
        (status = 400, description = "Invalid value"),
        (status = 404, description = "Provider not found")
    )
)]
pub async fn update_provider_params(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<Uuid>,
    Json(req): Json<UpdateProviderParamsRequest>,
) -> impl IntoResponse {
    // Ensure provider exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM providers WHERE id = $1)")
        .bind(provider_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);
    if !exists {
        return StatusCode::NOT_FOUND;
    }

    // Validate ranges (if provided)
    if let Some(v) = req.worker_instance_startup_timeout_s {
        if !validate_timeout_s(v) {
            return StatusCode::BAD_REQUEST;
        }
    }
    if let Some(v) = req.instance_startup_timeout_s {
        if !validate_timeout_s(v) {
            return StatusCode::BAD_REQUEST;
        }
    }

    // Upsert or delete (null => delete override)
    let mut tx = match state.db.begin().await {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    // WORKER_INSTANCE_STARTUP_TIMEOUT_S
    if let Some(v) = req.worker_instance_startup_timeout_s {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int)
            VALUES ($1, 'WORKER_INSTANCE_STARTUP_TIMEOUT_S', $2)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_int = EXCLUDED.value_int
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query(
            "DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_INSTANCE_STARTUP_TIMEOUT_S'",
        )
        .bind(provider_id)
        .execute(&mut *tx)
        .await;
    }

    // INSTANCE_STARTUP_TIMEOUT_S
    if let Some(v) = req.instance_startup_timeout_s {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int)
            VALUES ($1, 'INSTANCE_STARTUP_TIMEOUT_S', $2)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_int = EXCLUDED.value_int
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query(
            "DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'INSTANCE_STARTUP_TIMEOUT_S'",
        )
        .bind(provider_id)
        .execute(&mut *tx)
        .await;
    }

    if tx.commit().await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::OK
}


