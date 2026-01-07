// Models handlers
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use inventiv_common::LlmModel;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Postgres;
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::IntoParams;

use crate::app::AppState;

#[derive(Deserialize, IntoParams, utoipa::ToSchema)]
pub struct ListModelsParams {
    pub active: Option<bool>,
    /// Optional sort field (allowlist).
    pub order_by: Option<String>,
    /// "asc" | "desc"
    pub order_dir: Option<String>,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct CreateModelRequest {
    pub name: String,
    /// Hugging Face model repo id (or local path)
    pub model_id: String,
    pub required_vram_gb: i32,
    pub context_length: i32,
    pub is_active: Option<bool>,
    /// Recommended data volume size (GB) for this model (optional).
    pub data_volume_gb: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct UpdateModelRequest {
    pub name: Option<String>,
    pub model_id: Option<String>,
    pub required_vram_gb: Option<i32>,
    pub context_length: Option<i32>,
    pub is_active: Option<bool>,
    pub data_volume_gb: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

#[utoipa::path(
    get,
    path = "/models",
    params(ListModelsParams),
    responses((status = 200, description = "List models", body = [inventiv_common::LlmModel]))
)]
pub async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListModelsParams>,
) -> impl IntoResponse {
    let dir = match params
        .order_dir
        .as_deref()
        .unwrap_or("asc")
        .to_ascii_lowercase()
        .as_str()
    {
        "desc" => "DESC",
        _ => "ASC",
    };
    let order_by = match params.order_by.as_deref() {
        Some("model_id") => "model_id",
        Some("required_vram_gb") => "required_vram_gb",
        Some("context_length") => "context_length",
        Some("data_volume_gb") => "data_volume_gb",
        Some("is_active") => "is_active",
        Some("created_at") => "created_at",
        Some("updated_at") => "updated_at",
        _ => "name",
    };

    let base = r#"SELECT id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at
                 FROM models"#;
    let where_clause = if params.active == Some(true) {
        " WHERE is_active = true"
    } else {
        ""
    };
    let sql = format!(
        r#"{base}{where_clause}
           ORDER BY {order_by} {dir}, id {dir}"#
    );

    let rows: Vec<LlmModel> = sqlx::query_as(&sql)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();
    (StatusCode::OK, Json(rows)).into_response()
}

#[utoipa::path(
    get,
    path = "/instance_types/{instance_type_id}/models",
    params(
        ("instance_type_id" = uuid::Uuid, Path, description = "Instance type UUID")
    ),
    responses(
        (status = 200, description = "List compatible models for this instance type", body = [inventiv_common::LlmModel])
    )
)]
pub async fn list_compatible_models(
    State(state): State<Arc<AppState>>,
    Path(instance_type_id): Path<uuid::Uuid>,
) -> Json<Vec<inventiv_common::LlmModel>> {
    // List all active models compatible with this instance type
    let models = sqlx::query_as::<Postgres, inventiv_common::LlmModel>(
        r#"
        SELECT DISTINCT
            m.id, m.name, m.model_id, m.required_vram_gb, m.context_length,
            m.is_active, m.data_volume_gb, m.metadata, m.created_at, m.updated_at
        FROM models m
        WHERE m.is_active = true
          AND check_model_instance_compatibility(m.id, $1) = true
        ORDER BY m.name
        "#
    )
    .bind(instance_type_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);
    
    Json(models)
}

#[utoipa::path(
    get,
    path = "/models/{id}",
    responses((status = 200, description = "Get model", body = inventiv_common::LlmModel))
)]
pub async fn get_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uid) = uuid::Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_id"}))).into_response();
    };
    let row: Option<LlmModel> = sqlx::query_as(
        r#"SELECT id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at
           FROM models WHERE id = $1"#,
    )
    .bind(uid)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);
    match row {
        Some(m) => (StatusCode::OK, Json(m)).into_response(),
        None => (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/models",
    request_body = CreateModelRequest,
    responses((status = 201, description = "Created", body = inventiv_common::LlmModel))
)]
pub async fn create_model(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateModelRequest>,
) -> impl IntoResponse {
    let id = uuid::Uuid::new_v4();
    let is_active = payload.is_active.unwrap_or(true);
    let metadata = sqlx::types::Json(payload.metadata.unwrap_or_else(|| json!({})));
    let res: Result<LlmModel, sqlx::Error> = sqlx::query_as(
        r#"INSERT INTO models (id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NOW(),NOW())
           RETURNING id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at"#,
    )
    .bind(id)
    .bind(payload.name)
    .bind(payload.model_id)
    .bind(payload.required_vram_gb)
    .bind(payload.context_length)
    .bind(is_active)
    .bind(payload.data_volume_gb)
    .bind(metadata)
    .fetch_one(&state.db)
    .await;
    match res {
        Ok(m) => (StatusCode::CREATED, Json(m)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/models/{id}",
    request_body = UpdateModelRequest,
    responses((status = 200, description = "Updated", body = inventiv_common::LlmModel))
)]
pub async fn update_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateModelRequest>,
) -> impl IntoResponse {
    let Ok(uid) = uuid::Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_id"}))).into_response();
    };
    let metadata = payload.metadata.map(sqlx::types::Json);
    let row: Result<LlmModel, sqlx::Error> = sqlx::query_as(
        r#"UPDATE models
           SET name = COALESCE($2, name),
               model_id = COALESCE($3, model_id),
               required_vram_gb = COALESCE($4, required_vram_gb),
               context_length = COALESCE($5, context_length),
               is_active = COALESCE($6, is_active),
               data_volume_gb = COALESCE($7, data_volume_gb),
               metadata = COALESCE($8, metadata),
               updated_at = NOW()
           WHERE id = $1
           RETURNING id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at"#,
    )
    .bind(uid)
    .bind(payload.name)
    .bind(payload.model_id)
    .bind(payload.required_vram_gb)
    .bind(payload.context_length)
    .bind(payload.is_active)
    .bind(payload.data_volume_gb)
    .bind(metadata)
    .fetch_one(&state.db)
    .await;
    match row {
        Ok(m) => (StatusCode::OK, Json(m)).into_response(),
        Err(sqlx::Error::RowNotFound) => {
            (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":"db_error","message": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Serialize, utoipa::ToSchema)]
struct RecommendedDataVolumeResponse {
    model_id: String,
    model_name: String,
    recommended_data_volume_gb: i64,
    default_gb: i64,
    stored_data_volume_gb: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/models/{id}/recommended-data-volume",
    params(
        ("id" = uuid::Uuid, Path, description = "Model UUID"),
        ("provider_id" = Option<Uuid>, Query, description = "Optional provider ID to use provider-specific default")
    ),
    responses(
        (status = 200, description = "Recommended data volume size", body = RecommendedDataVolumeResponse),
        (status = 404, description = "Model not found")
    )
)]
pub async fn get_recommended_data_volume(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let Ok(uid) = uuid::Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_id"}))).into_response();
    };
    
    // Get model from DB
    let model: Option<LlmModel> = sqlx::query_as(
        r#"SELECT id, name, model_id, required_vram_gb, context_length, is_active, data_volume_gb, metadata, created_at, updated_at
           FROM models WHERE id = $1"#,
    )
    .bind(uid)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);
    
    let Some(model) = model else {
        return (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response();
    };
    
    // Get default_gb from provider settings or env
    let default_gb: i64 = if let Some(provider_id_str) = params.get("provider_id") {
        if let Ok(provider_id) = uuid::Uuid::parse_str(provider_id_str) {
            sqlx::query_scalar("SELECT value_int FROM provider_settings WHERE provider_id = $1 AND key = 'WORKER_DATA_VOLUME_GB_DEFAULT'")
                .bind(provider_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| {
                    std::env::var("WORKER_DATA_VOLUME_GB_DEFAULT")
                        .ok()
                        .and_then(|v| v.trim().parse::<i64>().ok())
                        .filter(|gb| *gb > 0)
                        .unwrap_or(200)
                })
        } else {
            std::env::var("WORKER_DATA_VOLUME_GB_DEFAULT")
                .ok()
                .and_then(|v| v.trim().parse::<i64>().ok())
                .filter(|gb| *gb > 0)
                .unwrap_or(200)
        }
    } else {
        std::env::var("WORKER_DATA_VOLUME_GB_DEFAULT")
            .ok()
            .and_then(|v| v.trim().parse::<i64>().ok())
            .filter(|gb| *gb > 0)
            .unwrap_or(200)
    };
    
    // Calculate recommended size using centralized logic
    let recommended_gb = inventiv_common::worker_storage::recommended_data_volume_gb(&model.model_id, default_gb)
        .unwrap_or(default_gb);
    
    let response = RecommendedDataVolumeResponse {
        model_id: model.model_id.clone(),
        model_name: model.name.clone(),
        recommended_data_volume_gb: recommended_gb,
        default_gb,
        stored_data_volume_gb: model.data_volume_gb,
    };
    
    (StatusCode::OK, Json(response)).into_response()
}

#[utoipa::path(
    delete,
    path = "/models/{id}",
    responses((status = 200, description = "Deleted"))
)]
pub async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Ok(uid) = uuid::Uuid::parse_str(&id) else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid_id"}))).into_response();
    };
    let res = sqlx::query("DELETE FROM models WHERE id = $1")
        .bind(uid)
        .execute(&state.db)
        .await;
    match res {
        Ok(r) if r.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"status":"ok"}))).into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, Json(json!({"error":"not_found"}))).into_response(),
        Err(e) => {
            // Most likely FK violation if instances still reference this model.
            let msg = e.to_string();
            let code = if msg.contains("foreign key") {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };
            (code, Json(json!({"error":"db_error","message": msg}))).into_response()
        }
    }
}

