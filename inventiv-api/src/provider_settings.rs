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
    pub default_bool: Option<bool>,
    pub default_text: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct ProviderParamsRow {
    pub provider_id: Uuid,
    pub provider_name: String,
    pub provider_code: String,
    pub worker_instance_startup_timeout_s: Option<i64>,
    pub instance_startup_timeout_s: Option<i64>,
    pub worker_ssh_bootstrap_timeout_s: Option<i64>,
    pub worker_health_port: Option<i64>,
    pub worker_vllm_port: Option<i64>,
    pub worker_data_volume_gb_default: Option<i64>,
    pub worker_expose_ports: Option<bool>,
    pub worker_vllm_mode: Option<String>,
    pub worker_vllm_image: Option<String>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateProviderParamsRequest {
    /// If null/missing, falls back to env/default (removes provider override).
    pub worker_instance_startup_timeout_s: Option<i64>,
    /// If null/missing, falls back to env/default (removes provider override).
    pub instance_startup_timeout_s: Option<i64>,
    pub worker_ssh_bootstrap_timeout_s: Option<i64>,
    pub worker_health_port: Option<i64>,
    pub worker_vllm_port: Option<i64>,
    pub worker_data_volume_gb_default: Option<i64>,
    pub worker_expose_ports: Option<bool>,
    pub worker_vllm_mode: Option<String>,
    pub worker_vllm_image: Option<String>,
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
        SELECT key, scope, value_type, min_int, max_int, default_int, default_bool, default_text, description
        FROM settings_definitions
        ORDER BY key
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}

#[derive(Debug, Serialize, Deserialize, FromRow, utoipa::ToSchema)]
pub struct GlobalSettingRow {
    pub key: String,
    pub value_type: String,
    pub value_int: Option<i64>,
    pub value_bool: Option<bool>,
    pub value_text: Option<String>,
    pub value_json: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpsertGlobalSettingRequest {
    pub key: String,
    /// Provide exactly one typed value for the key. Use null to delete override.
    pub value_int: Option<i64>,
    pub value_bool: Option<bool>,
    pub value_text: Option<String>,
    pub value_json: Option<serde_json::Value>,
}

#[utoipa::path(
    get,
    path = "/settings/global",
    tag = "Settings",
    responses((status = 200, description = "Global settings (overrides)", body = Vec<GlobalSettingRow>))
)]
pub async fn list_global_settings(State(state): State<Arc<AppState>>) -> Json<Vec<GlobalSettingRow>> {
    let rows = sqlx::query_as::<_, GlobalSettingRow>(
        r#"
        SELECT
          d.key,
          d.value_type,
          g.value_int,
          g.value_bool,
          g.value_text,
          g.value_json
        FROM settings_definitions d
        LEFT JOIN global_settings g ON g.key = d.key
        WHERE d.scope = 'global'
        ORDER BY d.key
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    Json(rows)
}

#[utoipa::path(
    put,
    path = "/settings/global",
    tag = "Settings",
    request_body = UpsertGlobalSettingRequest,
    responses(
        (status = 200, description = "Updated"),
        (status = 400, description = "Invalid value")
    )
)]
pub async fn upsert_global_setting(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpsertGlobalSettingRequest>,
) -> impl IntoResponse {
    let key = req.key.trim().to_string();
    if key.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    // Delete override if all values are null.
    if req.value_int.is_none() && req.value_bool.is_none() && req.value_text.is_none() && req.value_json.is_none() {
        let _ = sqlx::query("DELETE FROM global_settings WHERE key = $1")
            .bind(&key)
            .execute(&state.db)
            .await;
        return StatusCode::OK;
    }

    // Upsert (DB trigger validates scope/type/bounds)
    let res = sqlx::query(
        r#"
        INSERT INTO global_settings (key, value_int, value_bool, value_text, value_json)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (key) DO UPDATE SET
          value_int = EXCLUDED.value_int,
          value_bool = EXCLUDED.value_bool,
          value_text = EXCLUDED.value_text,
          value_json = EXCLUDED.value_json
        "#,
    )
    .bind(&key)
    .bind(req.value_int)
    .bind(req.value_bool)
    .bind(req.value_text.as_deref())
    .bind(req.value_json)
    .execute(&state.db)
    .await;

    match res {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::BAD_REQUEST,
    }
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
          MAX(CASE WHEN s.key = 'INSTANCE_STARTUP_TIMEOUT_S' THEN s.value_int END) AS instance_startup_timeout_s,
          MAX(CASE WHEN s.key = 'WORKER_SSH_BOOTSTRAP_TIMEOUT_S' THEN s.value_int END) AS worker_ssh_bootstrap_timeout_s,
          MAX(CASE WHEN s.key = 'WORKER_HEALTH_PORT' THEN s.value_int END) AS worker_health_port,
          MAX(CASE WHEN s.key = 'WORKER_VLLM_PORT' THEN s.value_int END) AS worker_vllm_port,
          MAX(CASE WHEN s.key = 'WORKER_DATA_VOLUME_GB_DEFAULT' THEN s.value_int END) AS worker_data_volume_gb_default,
          MAX(CASE WHEN s.key = 'WORKER_EXPOSE_PORTS' THEN s.value_bool END) AS worker_expose_ports,
          MAX(CASE WHEN s.key = 'WORKER_VLLM_MODE' THEN s.value_text END) AS worker_vllm_mode,
          MAX(CASE WHEN s.key = 'WORKER_VLLM_IMAGE' THEN s.value_text END) AS worker_vllm_image
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
    if let Some(v) = req.worker_ssh_bootstrap_timeout_s {
        if !validate_timeout_s(v) {
            return StatusCode::BAD_REQUEST;
        }
    }
    if let Some(v) = req.worker_health_port {
        if v < 1 || v > 65535 {
            return StatusCode::BAD_REQUEST;
        }
    }
    if let Some(v) = req.worker_vllm_port {
        if v < 1 || v > 65535 {
            return StatusCode::BAD_REQUEST;
        }
    }
    if let Some(v) = req.worker_data_volume_gb_default {
        if v < 50 || v > 5000 {
            return StatusCode::BAD_REQUEST;
        }
    }
    if let Some(m) = req.worker_vllm_mode.as_deref() {
        let m = m.trim().to_ascii_lowercase();
        if m != "mono" && m != "multi" {
            return StatusCode::BAD_REQUEST;
        }
    }
    if let Some(img) = req.worker_vllm_image.as_deref() {
        if img.trim().is_empty() {
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
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'WORKER_INSTANCE_STARTUP_TIMEOUT_S', $2, NULL, NULL, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_int = EXCLUDED.value_int, value_bool = NULL, value_text = NULL, value_json = NULL
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
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'INSTANCE_STARTUP_TIMEOUT_S', $2, NULL, NULL, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_int = EXCLUDED.value_int, value_bool = NULL, value_text = NULL, value_json = NULL
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

    // WORKER_SSH_BOOTSTRAP_TIMEOUT_S
    if let Some(v) = req.worker_ssh_bootstrap_timeout_s {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'WORKER_SSH_BOOTSTRAP_TIMEOUT_S', $2, NULL, NULL, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_int = EXCLUDED.value_int, value_bool = NULL, value_text = NULL, value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query(
            "DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_SSH_BOOTSTRAP_TIMEOUT_S'",
        )
        .bind(provider_id)
        .execute(&mut *tx)
        .await;
    }

    // WORKER_HEALTH_PORT
    if let Some(v) = req.worker_health_port {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'WORKER_HEALTH_PORT', $2, NULL, NULL, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_int = EXCLUDED.value_int, value_bool = NULL, value_text = NULL, value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query("DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_HEALTH_PORT'")
            .bind(provider_id)
            .execute(&mut *tx)
            .await;
    }

    // WORKER_VLLM_PORT
    if let Some(v) = req.worker_vllm_port {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'WORKER_VLLM_PORT', $2, NULL, NULL, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_int = EXCLUDED.value_int, value_bool = NULL, value_text = NULL, value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query("DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_PORT'")
            .bind(provider_id)
            .execute(&mut *tx)
            .await;
    }

    // WORKER_DATA_VOLUME_GB_DEFAULT
    if let Some(v) = req.worker_data_volume_gb_default {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'WORKER_DATA_VOLUME_GB_DEFAULT', $2, NULL, NULL, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_int = EXCLUDED.value_int, value_bool = NULL, value_text = NULL, value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query("DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_DATA_VOLUME_GB_DEFAULT'")
            .bind(provider_id)
            .execute(&mut *tx)
            .await;
    }

    // WORKER_EXPOSE_PORTS
    if let Some(v) = req.worker_expose_ports {
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'WORKER_EXPOSE_PORTS', NULL, $2, NULL, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_bool = EXCLUDED.value_bool, value_int = NULL, value_text = NULL, value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query("DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_EXPOSE_PORTS'")
            .bind(provider_id)
            .execute(&mut *tx)
            .await;
    }

    // WORKER_VLLM_MODE
    if let Some(v) = req.worker_vllm_mode.as_deref() {
        let v = v.trim().to_ascii_lowercase();
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'WORKER_VLLM_MODE', NULL, NULL, $2, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_text = EXCLUDED.value_text, value_int = NULL, value_bool = NULL, value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query("DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_MODE'")
            .bind(provider_id)
            .execute(&mut *tx)
            .await;
    }

    // WORKER_VLLM_IMAGE
    if let Some(v) = req.worker_vllm_image.as_deref() {
        let v = v.trim().to_string();
        let _ = sqlx::query(
            r#"
            INSERT INTO provider_settings (provider_id, key, value_int, value_bool, value_text, value_json)
            VALUES ($1, 'WORKER_VLLM_IMAGE', NULL, NULL, $2, NULL)
            ON CONFLICT (provider_id, key) DO UPDATE SET value_text = EXCLUDED.value_text, value_int = NULL, value_bool = NULL, value_json = NULL
            "#,
        )
        .bind(provider_id)
        .bind(v)
        .execute(&mut *tx)
        .await;
    } else {
        let _ = sqlx::query("DELETE FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_VLLM_IMAGE'")
            .bind(provider_id)
            .execute(&mut *tx)
            .await;
    }

    if tx.commit().await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    StatusCode::OK
}


