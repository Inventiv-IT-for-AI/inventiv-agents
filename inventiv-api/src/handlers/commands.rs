// Commands handlers (reconcile, catalog sync, action logs)
use axum::extract::{Query, State};
use axum::Json;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Postgres;
use std::sync::Arc;
use utoipa::IntoParams;

use crate::app::AppState;

/// POST /reconcile - Trigger manual reconciliation
#[utoipa::path(
    post,
    path = "/reconcile",
    responses(
        (status = 200, description = "Reconciliation triggered", body = serde_json::Value),
        (status = 500, description = "Failed to trigger reconciliation", body = serde_json::Value)
    )
)]
pub async fn manual_reconcile_trigger(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    println!("🔍 Manual reconciliation triggered via API");

    // Publish Redis event for orchestrator
    let event_payload = serde_json::json!({
        "type": "CMD:RECONCILE"
    })
    .to_string();

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    // Use turbofish to specify return type as unit ()
    match conn
        .publish::<_, _, ()>("orchestrator_events", &event_payload)
        .await
    {
        Ok(_) => Json(json!({
            "status": "triggered",
            "message": "Reconciliation task has been triggered"
        })),
        Err(e) => {
            eprintln!("Failed to publish reconciliation event: {:?}", e);
            Json(json!({
                "status": "error",
                "message": format!("Failed to trigger reconciliation: {:?}", e)
            }))
        }
    }
}

/// POST /catalog/sync - Trigger catalog synchronization
#[utoipa::path(
    post,
    path = "/catalog/sync",
    responses(
        (status = 200, description = "Catalog Sync triggered", body = serde_json::Value),
        (status = 500, description = "Failed to trigger sync", body = serde_json::Value)
    )
)]
pub async fn manual_catalog_sync_trigger(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    println!("🔄 Catalog Sync triggered via API");

    // Publish Redis event for orchestrator
    let event_payload = serde_json::json!({
        "type": "CMD:SYNC_CATALOG"
    })
    .to_string();

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap();
    // Use turbofish to specify return type as unit ()
    match conn
        .publish::<_, _, ()>("orchestrator_events", &event_payload)
        .await
    {
        Ok(_) => Json(json!({
            "status": "triggered",
            "message": "Catalog Sync task has been triggered"
        })),
        Err(e) => {
            eprintln!("Failed to publish sync event: {:?}", e);
            Json(json!({
                "status": "error",
                "message": format!("Failed to trigger sync: {:?}", e)
            }))
        }
    }
}

#[derive(Deserialize, IntoParams)]
pub struct ActionLogQuery {
    instance_id: Option<uuid::Uuid>,
    component: Option<String>,
    status: Option<String>,
    action_type: Option<String>,
    limit: Option<i32>,
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ActionLogResponse {
    id: uuid::Uuid,
    action_type: String,
    component: String,
    status: String,
    error_message: Option<String>,
    instance_id: Option<uuid::Uuid>,
    duration_ms: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
    metadata: Option<serde_json::Value>, // Added metadata field
    instance_status_before: Option<String>,
    instance_status_after: Option<String>,
}

#[utoipa::path(
    get,
    path = "/action_logs",
    params(ActionLogQuery),
    responses(
        (status = 200, description = "List of action logs", body = Vec<ActionLogResponse>)
    )
)]
pub async fn list_action_logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ActionLogQuery>,
) -> Json<Vec<ActionLogResponse>> {
    let limit = params.limit.unwrap_or(100).min(1000);

    let logs = sqlx::query_as::<Postgres, ActionLogResponse>(
        "SELECT 
            id, action_type, component, status, 
            error_message, instance_id, duration_ms, created_at, metadata,
            instance_status_before, instance_status_after
         FROM action_logs
         WHERE ($1::uuid IS NULL OR instance_id = $1)
           AND ($2::text IS NULL OR component = $2)
           AND ($3::text IS NULL OR status = $3)
           AND ($4::text IS NULL OR action_type = $4)
         ORDER BY created_at DESC
         LIMIT $5",
    )
    .bind(params.instance_id)
    .bind(params.component)
    .bind(params.status)
    .bind(params.action_type)
    .bind(limit as i64)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(logs)
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct ActionTypeResponse {
    code: String,
    label: String,
    icon: String,
    color_class: String,
    category: Option<String>,
    is_active: bool,
}

#[utoipa::path(
    get,
    path = "/action_types",
    responses(
        (status = 200, description = "List of action types", body = Vec<ActionTypeResponse>)
    )
)]
pub async fn list_action_types(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ActionTypeResponse>> {
    let rows = sqlx::query_as::<Postgres, ActionTypeResponse>(
        "SELECT code, label, icon, color_class, category, is_active
         FROM action_types
         WHERE is_active = true
         ORDER BY category NULLS LAST, code ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}
