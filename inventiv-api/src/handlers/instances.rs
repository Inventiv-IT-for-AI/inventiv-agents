use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::app::state::AppState;
use crate::progress;
use crate::simple_logger;
use redis::AsyncCommands;

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct InstanceResponse {
    pub id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub zone_id: Option<uuid::Uuid>,
    pub instance_type_id: Option<uuid::Uuid>,
    /// Provisioned model (catalog) selected at deployment time (optional).
    pub model_id: Option<uuid::Uuid>,
    pub model_name: Option<String>,
    /// Model code / HF repo id for the provisioned model (optional).
    pub model_code: Option<String>,
    pub provider_instance_id: Option<String>,
    pub status: String,
    pub ip_address: Option<String>,
    // Worker (data plane) state (optional; may be NULL when worker not registered yet)
    #[sqlx(default)]
    pub worker_status: Option<String>,
    #[sqlx(default)]
    pub worker_last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    pub worker_model_id: Option<String>,
    #[sqlx(default)]
    pub worker_queue_depth: Option<i32>,
    #[sqlx(default)]
    pub worker_gpu_utilization: Option<f64>,
    #[sqlx(default)]
    pub worker_health_port: Option<i32>,
    #[sqlx(default)]
    pub worker_vllm_port: Option<i32>,
    #[sqlx(default)]
    pub worker_metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub terminated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,
    pub last_reconciliation: Option<chrono::DateTime<chrono::Utc>>,
    pub health_check_failures: Option<i32>,
    pub deletion_reason: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    /// Count of attached block volumes (not deleted) tracked in DB.
    pub storage_count: i64,
    /// Attached block volume sizes in GB (not deleted) tracked in DB.
    pub storage_sizes_gb: Vec<i32>,

    // Joined Fields
    pub provider_name: String,
    pub region: String,
    pub zone: String,
    pub instance_type: String,
    pub cpu_count: Option<i32>,
    pub ram_gb: Option<i32>,
    pub gpu_vram: Option<i32>,
    pub gpu_count: Option<i32>, // NEW: Distinct GPU count
    pub cost_per_hour: Option<f64>,
    pub total_cost: Option<f64>,
    pub is_archived: bool,
    pub deleted_by_provider: Option<bool>,
    /// Progress percentage (0-100) towards operational state (calculated, not from DB)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(skip)]
    pub progress_percent: Option<u8>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct InstanceStorageInfo {
    // Identifiants
    pub id: uuid::Uuid,
    pub provider_volume_id: String,
    pub name: Option<String>,
    pub volume_type: String,
    pub size_gb: Option<i64>,
    pub is_boot: bool,

    // Statut et cycle de vie
    pub status: String, // 'attached', 'detached', 'deleting', 'deleted'
    pub delete_on_terminate: bool,

    // Timestamps (historique complet)
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub attached_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reconciled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_reconciliation: Option<chrono::DateTime<chrono::Utc>>,

    // Erreurs et réconciliation
    pub error_message: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct InstanceWithStoragesResponse {
    #[serde(flatten)]
    pub instance: InstanceResponse,
    pub storages: Vec<InstanceStorageInfo>,
}

#[derive(Deserialize, IntoParams)]
pub struct ListInstanceParams {
    pub archived: Option<bool>,
}

#[derive(Deserialize, IntoParams, utoipa::ToSchema)]
pub struct SearchInstancesParams {
    pub archived: Option<bool>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    /// Sort field allowlist: created_at|status|provider|region|zone|type|cost_per_hour|total_cost
    pub sort_by: Option<String>,
    /// "asc" | "desc"
    pub sort_dir: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SearchInstancesResponse {
    pub offset: i64,
    pub limit: i64,
    pub total_count: i64,
    pub filtered_count: i64,
    pub rows: Vec<InstanceResponse>,
}

#[utoipa::path(
    get,
    path = "/instances",
    params(ListInstanceParams),
    responses(
        (status = 200, description = "List all instances with details", body = Vec<InstanceResponse>)
    )
)]
pub async fn list_instances(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ListInstanceParams>,
) -> Json<Vec<InstanceResponse>> {
    let show_archived = params.archived.unwrap_or(false);

    let instances = sqlx::query_as::<Postgres, InstanceResponse>(
        r#"
        SELECT 
            i.id, i.provider_id, i.zone_id, i.instance_type_id,
            i.model_id,
            m.name as model_name,
            m.model_id as model_code,
            i.provider_instance_id::text as provider_instance_id,
            i.status::text as status, 
            i.ip_address::text as ip_address,
            i.worker_status,
            i.worker_last_heartbeat,
            i.worker_model_id,
            i.worker_queue_depth,
            i.worker_gpu_utilization,
            i.worker_health_port,
            i.worker_vllm_port,
            i.worker_metadata,
            i.created_at,
            i.terminated_at,
            i.last_health_check,
            (i.last_reconciliation AT TIME ZONE 'UTC') as last_reconciliation,
            i.health_check_failures,
            i.deletion_reason,
            i.error_code,
            i.error_message,
            COALESCE((SELECT COUNT(*) FROM instance_volumes iv WHERE iv.instance_id = i.id AND iv.deleted_at IS NULL), 0)::bigint as storage_count,
            COALESCE(
              (SELECT ARRAY_AGG(
                        (CASE
                          WHEN iv.size_bytes < 1000000000 THEN iv.size_bytes
                          ELSE ROUND(iv.size_bytes / 1000000000.0)
                         END)::int
                         ORDER BY
                         (CASE
                          WHEN iv.size_bytes < 1000000000 THEN iv.size_bytes
                          ELSE ROUND(iv.size_bytes / 1000000000.0)
                         END)::int
                      )
                 FROM instance_volumes iv
                WHERE iv.instance_id = i.id AND iv.deleted_at IS NULL AND iv.size_bytes > 0),
              ARRAY[]::int[]
            ) as storage_sizes_gb,
            i.is_archived,
            i.deleted_by_provider,
            COALESCE(p.name, 'Unknown Provider') as provider_name,
            COALESCE(z.name, 'Unknown Zone') as zone,
            COALESCE(r.name, 'Unknown Region') as region,
            COALESCE(it.name, 'Unknown Type') as instance_type,
            it.cpu_count as cpu_count,
            it.ram_gb as ram_gb,
            it.vram_per_gpu_gb as gpu_vram,
            it.gpu_count as gpu_count,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            (EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8) as total_cost
        FROM instances i
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN zones z ON i.zone_id = z.id
        LEFT JOIN regions r ON z.region_id = r.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
        LEFT JOIN models m ON m.id = i.model_id
        WHERE i.is_archived = $1
        ORDER BY i.created_at DESC
        "#
    )
    .bind(show_archived)
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    // Enrich instances with progress percentage
    let mut instances_vec = instances;
    progress::enrich_instances_with_progress(&state.db, &mut instances_vec).await;

    Json(instances_vec)
}

#[utoipa::path(
    get,
    path = "/instances/search",
    params(SearchInstancesParams),
    responses((status = 200, description = "Paged search instances (virtualized UI)", body = SearchInstancesResponse))
)]
pub async fn search_instances(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<SearchInstancesParams>,
) -> Json<SearchInstancesResponse> {
    let show_archived = params.archived.unwrap_or(false);
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(200).clamp(1, 500);
    let dir = match params
        .sort_dir
        .as_deref()
        .unwrap_or("desc")
        .to_ascii_lowercase()
        .as_str()
    {
        "asc" => "ASC",
        _ => "DESC",
    };

    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM instances")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let filtered_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instances WHERE is_archived = $1")
            .bind(show_archived)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

    let total_cost_expr = "(EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8)";
    let order_by = match params.sort_by.as_deref() {
        Some("status") => "i.status",
        Some("provider") => "p.name",
        Some("region") => "r.name",
        Some("zone") => "z.name",
        Some("type") => "it.name",
        Some("cost_per_hour") => "it.cost_per_hour",
        Some("total_cost") => total_cost_expr,
        _ => "i.created_at",
    };

    let sql = format!(
        r#"
        SELECT 
            i.id, i.provider_id, i.zone_id, i.instance_type_id,
            i.model_id,
            m.name as model_name,
            m.model_id as model_code,
            i.provider_instance_id::text as provider_instance_id,
            i.status::text as status, 
            i.ip_address::text as ip_address,
            i.worker_status,
            i.worker_last_heartbeat,
            i.worker_model_id,
            i.worker_queue_depth,
            i.worker_gpu_utilization,
            i.worker_health_port,
            i.worker_vllm_port,
            i.worker_metadata,
            i.created_at,
            i.terminated_at,
            i.last_health_check,
            (i.last_reconciliation AT TIME ZONE 'UTC') as last_reconciliation,
            i.health_check_failures,
            i.deletion_reason,
            i.error_code,
            i.error_message,
            COALESCE((SELECT COUNT(*) FROM instance_volumes iv WHERE iv.instance_id = i.id AND iv.deleted_at IS NULL), 0)::bigint as storage_count,
            COALESCE(
              (SELECT ARRAY_AGG(
                        (CASE
                          WHEN iv.size_bytes < 1000000000 THEN iv.size_bytes
                          ELSE ROUND(iv.size_bytes / 1000000000.0)
                         END)::int
                         ORDER BY
                         (CASE
                          WHEN iv.size_bytes < 1000000000 THEN iv.size_bytes
                          ELSE ROUND(iv.size_bytes / 1000000000.0)
                         END)::int
                      )
                 FROM instance_volumes iv
                WHERE iv.instance_id = i.id AND iv.deleted_at IS NULL AND iv.size_bytes > 0),
              ARRAY[]::int[]
            ) as storage_sizes_gb,
            i.is_archived,
            i.deleted_by_provider,
            COALESCE(p.name, 'Unknown Provider') as provider_name,
            COALESCE(z.name, 'Unknown Zone') as zone,
            COALESCE(r.name, 'Unknown Region') as region,
            COALESCE(it.name, 'Unknown Type') as instance_type,
            it.cpu_count as cpu_count,
            it.ram_gb as ram_gb,
            it.vram_per_gpu_gb as gpu_vram,
            it.gpu_count as gpu_count,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            {total_cost_expr} as total_cost
        FROM instances i
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN zones z ON i.zone_id = z.id
        LEFT JOIN regions r ON z.region_id = r.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
        LEFT JOIN models m ON m.id = i.model_id
        WHERE i.is_archived = $1
        ORDER BY {order_by} {dir} NULLS LAST, i.id {dir}
        LIMIT $2 OFFSET $3
        "#
    );

    let mut rows: Vec<InstanceResponse> = sqlx::query_as::<Postgres, InstanceResponse>(&sql)
        .bind(show_archived)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    // Enrich instances with progress percentage
    progress::enrich_instances_with_progress(&state.db, &mut rows).await;

    Json(SearchInstancesResponse {
        offset,
        limit,
        total_count,
        filtered_count,
        rows,
    })
}

#[utoipa::path(
    get,
    path = "/instances/{id}",
    params(
        ("id" = uuid::Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 200, description = "Instance details", body = InstanceWithStoragesResponse),
        (status = 404, description = "Instance not found")
    )
)]
pub async fn get_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<Postgres, InstanceResponse>(
        r#"
        SELECT 
            i.id, i.provider_id, i.zone_id, i.instance_type_id,
            i.model_id,
            m.name as model_name,
            m.model_id as model_code,
            i.provider_instance_id::text as provider_instance_id,
            i.status::text as status, 
            i.ip_address::text as ip_address,
            i.worker_status,
            i.worker_last_heartbeat,
            i.worker_model_id,
            i.worker_queue_depth,
            i.worker_gpu_utilization,
            i.worker_health_port,
            i.worker_vllm_port,
            i.worker_metadata,
            i.created_at,
            i.terminated_at,
            i.last_health_check,
            (i.last_reconciliation AT TIME ZONE 'UTC') as last_reconciliation,
            i.health_check_failures,
            i.deletion_reason,
            i.error_code,
            i.error_message,
            COALESCE((SELECT COUNT(*) FROM instance_volumes iv WHERE iv.instance_id = i.id AND iv.deleted_at IS NULL), 0)::bigint as storage_count,
            COALESCE(
              (SELECT ARRAY_AGG(
                        (CASE
                          WHEN iv.size_bytes < 1000000000 THEN iv.size_bytes
                          ELSE ROUND(iv.size_bytes / 1000000000.0)
                         END)::int
                         ORDER BY
                         (CASE
                          WHEN iv.size_bytes < 1000000000 THEN iv.size_bytes
                          ELSE ROUND(iv.size_bytes / 1000000000.0)
                         END)::int
                      )
                 FROM instance_volumes iv
                WHERE iv.instance_id = i.id AND iv.deleted_at IS NULL AND iv.size_bytes > 0),
              ARRAY[]::int[]
            ) as storage_sizes_gb,
            i.is_archived,
            i.deleted_by_provider,
            COALESCE(p.name, 'Unknown Provider') as provider_name,
            COALESCE(z.name, 'Unknown Zone') as zone,
            COALESCE(r.name, 'Unknown Region') as region,
            COALESCE(it.name, 'Unknown Type') as instance_type,
            it.cpu_count as cpu_count,
            it.ram_gb as ram_gb,
            it.vram_per_gpu_gb as gpu_vram,
            it.gpu_count as gpu_count,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            (EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8) as total_cost
        FROM instances i
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN zones z ON i.zone_id = z.id
        LEFT JOIN regions r ON z.region_id = r.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
        LEFT JOIN models m ON m.id = i.model_id
        WHERE i.id = $1
        LIMIT 1
        "#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some(mut inst) => {
            // Enrich instance with progress percentage
            progress::enrich_instances_with_progress(&state.db, std::slice::from_mut(&mut inst))
                .await;
            let storages: Vec<InstanceStorageInfo> = sqlx::query_as::<
                Postgres,
                (
                    uuid::Uuid,                            // id
                    String,                                // provider_volume_id
                    Option<String>,                        // provider_volume_name
                    String,                                // volume_type
                    i64,                                   // size_bytes
                    bool,                                  // is_boot
                    String,                                // status
                    bool,                                  // delete_on_terminate
                    chrono::DateTime<chrono::Utc>,         // created_at
                    Option<chrono::DateTime<chrono::Utc>>, // attached_at
                    Option<chrono::DateTime<chrono::Utc>>, // deleted_at
                    Option<chrono::DateTime<chrono::Utc>>, // reconciled_at
                    Option<chrono::DateTime<chrono::Utc>>, // last_reconciliation
                    Option<String>,                        // error_message
                ),
            >(
                r#"
                SELECT
                  iv.id,
                  iv.provider_volume_id,
                  iv.provider_volume_name,
                  iv.volume_type,
                  iv.size_bytes,
                  iv.is_boot,
                  iv.status,
                  iv.delete_on_terminate,
                  iv.created_at,
                  iv.attached_at,
                  iv.deleted_at,
                  iv.reconciled_at,
                  iv.last_reconciliation,
                  iv.error_message
                FROM instance_volumes iv
                WHERE iv.instance_id = $1
                ORDER BY 
                  -- Actifs en premier, puis par date de création décroissante
                  CASE WHEN iv.deleted_at IS NULL THEN 0 ELSE 1 END,
                  iv.created_at DESC
                "#,
            )
            .bind(id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(
                |(
                    id,
                    provider_volume_id,
                    name,
                    volume_type,
                    size_bytes,
                    is_boot,
                    status,
                    delete_on_terminate,
                    created_at,
                    attached_at,
                    deleted_at,
                    reconciled_at,
                    last_reconciliation,
                    error_message,
                )| {
                    InstanceStorageInfo {
                        id,
                        provider_volume_id,
                        name,
                        volume_type,
                        size_gb: if size_bytes > 0 {
                            if size_bytes < 1_000_000_000 {
                                // Some providers return "size" in GB already.
                                Some(size_bytes)
                            } else {
                                Some(((size_bytes as f64) / 1_000_000_000.0).round() as i64)
                            }
                        } else {
                            None
                        },
                        is_boot,
                        status,
                        delete_on_terminate,
                        created_at,
                        attached_at,
                        deleted_at,
                        reconciled_at,
                        last_reconciliation,
                        error_message,
                    }
                },
            )
            .collect();

            Json(InstanceWithStoragesResponse {
                instance: inst,
                storages,
            })
            .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Instance not found").into_response(),
    }
}

// Archive endpoint (logged version below)
// COMMAND : ARCHIVE INSTANCE
#[utoipa::path(
    put,
    path = "/instances/{id}/archive",
    params(
        ("id" = uuid::Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 200, description = "Instance Archived"),
        (status = 400, description = "Bad Request"),
        (status = 500, description = "Server Error")
    )
)]
pub async fn archive_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Log start of archive action
    let start = std::time::Instant::now();
    let log_id =
        simple_logger::log_action(&state.db, "ARCHIVE_INSTANCE", "in_progress", Some(id), None)
            .await
            .ok();

    let result = sqlx::query(
        "UPDATE instances
         SET is_archived = true,
             status = 'archived'
         WHERE id = $1
           AND status IN ('terminated', 'archived')",
    )
    .bind(id)
    .execute(&state.db)
    .await;

    let response = match result {
        Ok(r) if r.rows_affected() > 0 => (StatusCode::OK, "Instance Archived"),
        Ok(_) => (
            StatusCode::BAD_REQUEST,
            "Instance not found or not terminated",
        ),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database Error"),
    };

    // Log completion
    if let Some(lid) = log_id {
        let duration = start.elapsed().as_millis() as i32;
        let status_str = match response.0 {
            StatusCode::OK => "success",
            _ => "failed",
        };
        let err_msg = if response.0 == StatusCode::OK {
            None
        } else {
            Some(response.1)
        };
        simple_logger::log_action_complete(&state.db, lid, status_str, duration, err_msg)
            .await
            .ok();
    }

    response.into_response()
}

// COMMAND : TERMINATE INSTANCE
#[utoipa::path(
    delete,
    path = "/instances/{id}",
    params(
        ("id" = uuid::Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 202, description = "Termination Accepted")
    )
)]
pub async fn terminate_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Log start of archive action
    let start = std::time::Instant::now();
    let log_id = simple_logger::log_action_with_metadata(
        &state.db,
        "REQUEST_TERMINATE",
        "in_progress",
        Some(id),
        None,
        Some(serde_json::json!({
            "instance_id": id.to_string(),
        })),
    )
    .await
    .ok();

    println!("🗑️ Termination Request: {}", id);

    // 1. Fetch instance so we can handle edge-cases safely (no provider resource, missing zone, etc.)
    let instance_row: Option<(Option<String>, Option<uuid::Uuid>, String)> = sqlx::query_as(
        "SELECT provider_instance_id::text, zone_id, status::text FROM instances WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some((provider_instance_id_opt, zone_id_opt, status)) = instance_row else {
        println!("⚠️  Instance {} not found for termination", id);
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Instance not found"),
            )
            .await
            .ok();
        }
        return (StatusCode::NOT_FOUND, "Instance not found").into_response();
    };

    if status == "terminated" {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "success",
                duration,
                None,
                Some(serde_json::json!({"already_terminated": true})),
            )
            .await
            .ok();
        }
        return (StatusCode::OK, "Already terminated").into_response();
    }

    // If there is no provider resource to delete (provider_instance_id missing), we terminate immediately.
    // This prevents "terminating forever" for failed/invalid provisioning requests.
    //
    // IMPORTANT: if provider_instance_id exists but zone_id is missing, we must NOT mark terminated:
    // we can't safely call the provider API and risk leaking resources. We keep 'terminating' and let
    // admin/operator handle the missing catalog linkage.
    if provider_instance_id_opt.as_deref().unwrap_or("").is_empty() {
        let _ = sqlx::query(
            "UPDATE instances
             SET status='terminated',
                 terminated_at = COALESCE(terminated_at, NOW()),
                 deletion_reason = COALESCE(deletion_reason, 'no_provider_resource')
             WHERE id=$1 AND status != 'terminated'",
        )
        .bind(id)
        .execute(&state.db)
        .await;

        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "success",
                duration,
                None,
                Some(serde_json::json!({
                    "immediate": true,
                    "reason": "no_provider_resource",
                    "provider_instance_id_present": provider_instance_id_opt.is_some(),
                    "zone_id_present": zone_id_opt.is_some(),
                })),
            )
            .await
            .ok();
        }

        return (StatusCode::OK, "Terminated (no provider resource)").into_response();
    }

    if zone_id_opt.is_none() {
        // Can't safely terminate on provider without a zone -> keep terminating and surface an error.
        let _ = sqlx::query(
            "UPDATE instances
             SET status='terminating',
                 error_code = COALESCE(error_code, 'MISSING_ZONE'),
                 error_message = COALESCE(error_message, 'Missing zone for termination'),
                 last_reconciliation = NULL
             WHERE id=$1 AND status != 'terminated'",
        )
        .bind(id)
        .execute(&state.db)
        .await;

        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "success",
                duration,
                Some("Missing zone: kept terminating for manual recovery"),
                Some(serde_json::json!({
                    "immediate": false,
                    "reason": "missing_zone",
                    "provider_instance_id_present": provider_instance_id_opt.is_some(),
                    "zone_id_present": zone_id_opt.is_some(),
                })),
            )
            .await
            .ok();
        }

        // Still publish CMD:TERMINATE (best effort) in case orchestrator can reconcile other metadata,
        // but the terminator job will also pick it up via status='terminating'.
        // (We don't early-return here; continue to publish.)
    }

    // 2. Update status to 'terminating' in DB (provider resource exists, orchestrator will delete it)
    let update_result = sqlx::query(
        "UPDATE instances
         SET status = 'terminating',
             last_reconciliation = NULL
         WHERE id = $1 AND status != 'terminated'",
    )
    .bind(id)
    .execute(&state.db)
    .await;

    match update_result {
        Ok(result) if result.rows_affected() > 0 => {
            println!("✅ Instance {} status set to 'terminating'", id)
        }
        Ok(_) => {
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete(
                    &state.db,
                    log_id,
                    "failed",
                    duration,
                    Some("Instance not found"),
                )
                .await
                .ok();
            }
            return (StatusCode::NOT_FOUND, "Instance not found").into_response();
        }
        Err(e) => {
            println!("❌ Failed to update instance status: {:?}", e);
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                let msg = format!("Database error: {:?}", e);
                simple_logger::log_action_complete(
                    &state.db,
                    log_id,
                    "failed",
                    duration,
                    Some(&msg),
                )
                .await
                .ok();
            }
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    }

    // 3. Send termination event to orchestrator (async)
    let event = serde_json::json!({
        "type": "CMD:TERMINATE",
        "instance_id": id.to_string(),
        "correlation_id": log_id.map(|id| id.to_string()),
    })
    .to_string();

    println!("📤 Publishing termination event to Redis: {}", event);

    match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            match conn
                .publish::<_, _, ()>("orchestrator_events", &event)
                .await
            {
                Ok(_) => {
                    println!("✅ Termination event published successfully");
                    // Log success
                    if let Some(log_id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            log_id,
                            "success",
                            duration,
                            None,
                            Some(serde_json::json!({"redis_published": true, "event_type": "CMD:TERMINATE"})),
                        ).await.ok();
                    }
                    (StatusCode::ACCEPTED, "Termination initiated").into_response()
                }
                Err(e) => {
                    let error_msg = format!("Failed to publish to Redis: {:?}", e);
                    println!("❌ {}", error_msg);
                    if let Some(log_id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            log_id,
                            "failed",
                            duration,
                            Some(&error_msg),
                            Some(serde_json::json!({"redis_published": false, "event_type": "CMD:TERMINATE"})),
                        ).await.ok();
                    }
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to queue termination",
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to connect to Redis: {:?}", e);
            println!("❌ {}", error_msg);
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    log_id,
                    "failed",
                    duration,
                    Some(&error_msg),
                    Some(serde_json::json!({"redis_published": false, "event_type": "CMD:TERMINATE"})),
                ).await.ok();
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to queue termination",
            )
                .into_response()
        }
    }
}

// COMMAND : REINSTALL INSTANCE (force SSH bootstrap again)
#[utoipa::path(
    post,
    path = "/instances/{id}/reinstall",
    params(
        ("id" = uuid::Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 202, description = "Reinstall Accepted")
    )
)]
pub async fn reinstall_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let log_id = simple_logger::log_action_with_metadata(
        &state.db,
        "REQUEST_REINSTALL",
        "in_progress",
        Some(id),
        None,
        Some(serde_json::json!({
            "instance_id": id.to_string(),
        })),
    )
    .await
    .ok();

    // Validate instance exists and is eligible
    let instance_row: Option<(Option<String>, Option<String>, String)> = sqlx::query_as(
        "SELECT provider_instance_id::text, ip_address::text, status::text FROM instances WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let Some((provider_instance_id_opt, ip_opt, status)) = instance_row else {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Instance not found"),
            )
            .await
            .ok();
        }
        return (StatusCode::NOT_FOUND, "Instance not found").into_response();
    };

    if status == "terminated" {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Instance is terminated"),
                Some(serde_json::json!({"status": status})),
            )
            .await
            .ok();
        }
        return (StatusCode::BAD_REQUEST, "Instance is terminated").into_response();
    }
    if status == "terminating" {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Instance is terminating"),
                Some(serde_json::json!({"status": status})),
            )
            .await
            .ok();
        }
        return (StatusCode::CONFLICT, "Instance is terminating").into_response();
    }

    // Must have a reachable VM to reinstall.
    if provider_instance_id_opt.as_deref().unwrap_or("").is_empty()
        || ip_opt.as_deref().unwrap_or("").is_empty()
    {
        if let Some(log_id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                log_id,
                "failed",
                duration,
                Some("Missing provider_instance_id or ip_address"),
                Some(serde_json::json!({
                    "provider_instance_id_present": provider_instance_id_opt.as_deref().unwrap_or("").is_empty() == false,
                    "ip_address_present": ip_opt.as_deref().unwrap_or("").is_empty() == false,
                })),
            )
            .await
            .ok();
        }
        return (
            StatusCode::BAD_REQUEST,
            "Instance not reachable (missing provider_instance_id or ip_address)",
        )
            .into_response();
    }

    // Mark as booting again (repair workflow) to re-enable health-check flow.
    let _ = sqlx::query(
        "UPDATE instances
         SET status = 'booting',
             boot_started_at = NOW(),
             last_health_check = NULL,
             health_check_failures = 0,
             error_code = NULL,
             error_message = NULL,
             failed_at = NULL
         WHERE id = $1
           AND status NOT IN ('terminated', 'terminating')",
    )
    .bind(id)
    .execute(&state.db)
    .await;

    // Publish reinstall command to orchestrator
    let event = serde_json::json!({
        "type": "CMD:REINSTALL",
        "instance_id": id.to_string(),
        "correlation_id": log_id.map(|id| id.to_string()),
    })
    .to_string();

    match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => match conn
            .publish::<_, _, ()>("orchestrator_events", &event)
            .await
        {
            Ok(_) => {
                if let Some(log_id) = log_id {
                    let duration = start.elapsed().as_millis() as i32;
                    simple_logger::log_action_complete_with_metadata(
                        &state.db,
                        log_id,
                        "success",
                        duration,
                        None,
                        Some(serde_json::json!({"redis_published": true, "event_type": "CMD:REINSTALL"})),
                    )
                    .await
                    .ok();
                }
                (StatusCode::ACCEPTED, "Reinstall initiated").into_response()
            }
            Err(e) => {
                let error_msg = format!("Failed to publish to Redis: {:?}", e);
                if let Some(log_id) = log_id {
                    let duration = start.elapsed().as_millis() as i32;
                    simple_logger::log_action_complete_with_metadata(
                        &state.db,
                        log_id,
                        "failed",
                        duration,
                        Some(&error_msg),
                        Some(serde_json::json!({"redis_published": false, "event_type": "CMD:REINSTALL"})),
                    )
                    .await
                    .ok();
                }
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to queue reinstall",
                )
                    .into_response()
            }
        },
        Err(e) => {
            let error_msg = format!("Failed to connect to Redis: {:?}", e);
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    log_id,
                    "failed",
                    duration,
                    Some(&error_msg),
                    Some(serde_json::json!({"redis_published": false, "event_type": "CMD:REINSTALL"})),
                )
                .await
                .ok();
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to queue reinstall",
            )
                .into_response()
        }
    }
}
