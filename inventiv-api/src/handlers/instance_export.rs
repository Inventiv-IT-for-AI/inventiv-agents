use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use sqlx::Postgres;
use std::sync::Arc;
use uuid::Uuid;

use crate::app::state::AppState;
use crate::handlers::instances::{InstanceResponse, InstanceStorageInfo};
use crate::progress;
use crate::simple_logger;

/// Enhanced action log with progress and phase information
#[derive(Serialize, utoipa::ToSchema)]
pub struct EnhancedActionLog {
    // Original fields
    pub id: Uuid,
    pub action_type: String,
    pub component: String,
    pub status: String,
    pub provider_name: Option<String>,
    pub instance_type: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub instance_id: Option<Uuid>,
    pub duration_ms: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: Option<serde_json::Value>,
    pub instance_status_before: Option<String>,
    pub instance_status_after: Option<String>,

    // Enhanced fields
    /// Progress percentage at the time this action completed (0-100)
    pub progress_percent: Option<u8>,
    /// Phase this action belongs to (provisioning, booting, ready, terminating, terminated)
    pub phase: Option<String>,
    /// Sub-phase within the main phase (e.g., "docker_install", "vllm_start" within "booting")
    pub sub_phase: Option<String>,
    /// Retry attempt number (0 = first attempt, 1+ = retries)
    pub retry_count: Option<u32>,
    /// Whether this action represents a state transition
    pub is_state_transition: bool,
    /// Whether this transition is valid according to the state machine
    pub is_valid_transition: Option<bool>,
    /// Time elapsed since instance creation at action start (seconds)
    pub elapsed_seconds_since_start: Option<i64>,
    /// Time elapsed since instance creation at action completion (seconds)
    pub elapsed_seconds_since_start_completed: Option<i64>,
}

/// State transition history entry
#[derive(Serialize, utoipa::ToSchema)]
pub struct StateTransition {
    pub id: Uuid,
    pub from_status: Option<String>,
    pub to_status: String,
    pub reason: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Time elapsed since instance creation (seconds)
    pub elapsed_seconds_since_start: i64,
}

/// Phase summary with duration and action count
#[derive(Serialize, utoipa::ToSchema)]
pub struct PhaseSummary {
    pub phase: String,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub duration_seconds: Option<i64>,
    pub action_count: usize,
    pub success_count: usize,
    pub failed_count: usize,
    pub progress_start: Option<u8>,
    pub progress_end: Option<u8>,
}

/// Export summary with analysis
#[derive(Serialize, utoipa::ToSchema)]
pub struct ExportSummary {
    pub total_actions: usize,
    pub total_state_transitions: usize,
    pub phases: Vec<PhaseSummary>,
    pub total_duration_seconds: Option<i64>,
    pub invalid_transitions: Vec<String>,
    pub retry_count: usize,
    pub exported_at: chrono::DateTime<chrono::Utc>,
}

/// Enhanced export response
#[derive(Serialize, utoipa::ToSchema)]
pub struct InstanceExportResponse {
    pub instance: InstanceResponse,
    pub storages: Vec<InstanceStorageInfo>,
    pub actions: Vec<EnhancedActionLog>,
    pub state_transitions: Vec<StateTransition>,
    pub summary: ExportSummary,
}

/// Valid state machine transitions
const VALID_TRANSITIONS: &[(&str, &[&str])] = &[
    ("provisioning", &["booting", "provisioning_failed"]),
    ("booting", &["installing", "starting", "ready", "startup_failed", "unavailable"]),
    ("installing", &["starting", "ready", "startup_failed"]),
    ("starting", &["ready", "startup_failed", "unavailable"]),
    ("ready", &["draining", "terminating", "unavailable", "terminated"]),
    ("draining", &["terminating"]),
    ("terminating", &["terminated"]),
    ("terminated", &["archived"]),
    ("unavailable", &["ready", "terminating", "terminated"]),
    ("provisioning_failed", &[]),
    ("startup_failed", &["booting", "terminating", "terminated"]),
    ("archived", &[]),
];

/// Check if a state transition is valid
fn is_valid_state_transition(from: Option<&str>, to: &str) -> bool {
    let from = match from {
        Some(s) => s,
        None => return true, // First transition is always valid
    };

    if from == to {
        return true; // Same state is valid (idempotent)
    }

    for (valid_from, valid_tos) in VALID_TRANSITIONS {
        if from == *valid_from {
            return valid_tos.contains(&to);
        }
    }

    false
}

/// Determine phase from action type and status
/// Priority: action_type > instance_status (action type is more reliable indicator)
/// 
/// Phases:
/// - provisioning: Instance creation at provider (0-25%)
/// - booting: Instance starting, SSH becoming accessible (25-50%)
/// - installing: Docker and model installation via SSH (50-60%)
/// - starting: Containers starting, model loading, warmup, health checks (60-95%)
/// - ready: Instance fully operational (100%)
fn determine_phase(action_type: &str, instance_status: Option<&str>) -> Option<String> {
    // First, check action type (more reliable indicator of what phase the action belongs to)
    match action_type {
        // Provisioning phase actions (0-25%)
        "REQUEST_CREATE" | "EXECUTE_CREATE" | "PROVIDER_CREATE" | "PROVIDER_VOLUME_RESIZE" 
        | "PERSIST_PROVIDER_ID" | "REQUEUE_PROVISION" => {
            return Some("provisioning".to_string());
        }
        // Booting phase actions (25-50%) - Instance starting, SSH becoming accessible
        "PROVIDER_START" | "PROVIDER_GET_IP" | "PROVIDER_SECURITY_GROUP" | "WORKER_SSH_ACCESSIBLE" 
        | "INSTANCE_CREATED" => {
            return Some("booting".to_string());
        }
        // Installing phase actions (50-60%) - Docker and model installation
        "WORKER_SSH_INSTALL" => {
            return Some("installing".to_string());
        }
        // Starting phase actions (60-95%) - Containers starting, model loading, warmup, health checks
        "WORKER_VLLM_HTTP_OK" | "WORKER_MODEL_LOADED" | "WORKER_VLLM_WARMUP" | "HEALTH_CHECK" => {
            return Some("starting".to_string());
        }
        // Ready phase actions (100%)
        "INSTANCE_READY" => {
            return Some("ready".to_string());
        }
        // Termination phase actions
        "REQUEST_TERMINATE" | "EXECUTE_TERMINATE" | "PROVIDER_TERMINATE" | "PROVIDER_DELETE_VOLUME" 
        | "PROVIDER_DELETE" | "INSTANCE_TERMINATED" | "RECOVERY_TERMINATE" 
        | "VOLUME_RECONCILIATION_RETRY_DELETE" => {
            return Some("terminating".to_string());
        }
        // For other actions (like GET_INSTANCE), use instance status as fallback
        _ => {
            let status = instance_status.unwrap_or("");
            if !status.is_empty() {
                match status {
                    "provisioning" | "provisioning_failed" => return Some("provisioning".to_string()),
                    "booting" => return Some("booting".to_string()),
                    "installing" => return Some("installing".to_string()),
                    "starting" | "startup_failed" => return Some("starting".to_string()),
                    "ready" | "unavailable" => return Some("ready".to_string()),
                    "draining" => return Some("draining".to_string()),
                    "terminating" => return Some("terminating".to_string()),
                    "terminated" | "archived" => return Some("terminated".to_string()),
                    _ => {}
                }
            }
        }
    }
    
    None
}

/// Extract sub-phase from metadata
fn extract_sub_phase(metadata: &Option<serde_json::Value>) -> Option<String> {
    metadata.as_ref().and_then(|m| {
        m.get("last_phase")
            .or_else(|| m.get("phase"))
            .or_else(|| m.get("phases").and_then(|p| p.as_array().and_then(|arr| arr.last())))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    })
}

/// Calculate progress percentage for a specific action based on action type
/// This provides a more accurate progress estimate based on the action's position in the lifecycle
fn calculate_action_progress_by_type(
    action_type: &str,
    status: &str,
    instance_status_after: Option<&str>,
) -> Option<u8> {
    // Use instance status after action if available, otherwise use action status
    let effective_status = instance_status_after.unwrap_or(status);

    // Map action types to progress percentages based on the documented progress stages
    match action_type {
        // Provisioning phase (0-25%)
        "REQUEST_CREATE" => Some(5),
        "EXECUTE_CREATE" => Some(10),
        "PROVIDER_CREATE" => Some(20),
        "PROVIDER_VOLUME_RESIZE" => Some(25),
        "PERSIST_PROVIDER_ID" => Some(20),
        "REQUEUE_PROVISION" => Some(15), // Re-queued provisioning (between REQUEST_CREATE and PROVIDER_CREATE)

        // Booting phase (25-50%) - Instance starting, SSH becoming accessible
        "PROVIDER_START" => Some(30),
        "PROVIDER_GET_IP" => Some(40),
        "PROVIDER_SECURITY_GROUP" => Some(45),
        "WORKER_SSH_ACCESSIBLE" => Some(50),
        "INSTANCE_CREATED" => Some(25),
        
        // Installing phase (50-60%) - Docker and model installation via SSH
        "WORKER_SSH_INSTALL" => {
            // Check if completed successfully
            if status == "success" {
                Some(60) // Installation complete
            } else {
                Some(55) // Installation in progress
            }
        }
        
        // Starting phase (60-95%) - Containers starting, model loading, warmup, health checks
        "WORKER_VLLM_HTTP_OK" => Some(70),
        "WORKER_MODEL_LOADED" => Some(80),
        "WORKER_VLLM_WARMUP" => Some(90),
        "HEALTH_CHECK" => {
            if status == "success" {
                Some(95)
            } else {
                Some(90) // Waiting for health checks
            }
        }
        
        // Ready phase (100%)
        "INSTANCE_READY" => Some(100),

        // Termination phase
        "REQUEST_TERMINATE" | "EXECUTE_TERMINATE" | "PROVIDER_TERMINATE" 
        | "PROVIDER_DELETE_VOLUME" | "PROVIDER_DELETE" | "INSTANCE_TERMINATED" 
        | "RECOVERY_TERMINATE" | "VOLUME_RECONCILIATION_RETRY_DELETE" => {
            // Termination actions don't contribute to progress (instance is being destroyed)
            Some(0)
        }

        // Other actions - use status-based estimation
        _ => {
            match effective_status {
                "provisioning" => Some(15),
                "booting" => Some(40), // Between PROVIDER_START and WORKER_SSH_ACCESSIBLE
                "installing" => Some(55), // Installation in progress
                "starting" => Some(70), // Between WORKER_SSH_INSTALL and WORKER_VLLM_HTTP_OK
                "ready" => Some(100),
                "terminating" | "terminated" | "archived" => Some(0),
                _ => None,
            }
        }
    }
}


#[utoipa::path(
    get,
    path = "/instances/{id}/export",
    params(
        ("id" = uuid::Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 200, description = "Instance export with enhanced action logs", body = InstanceExportResponse),
        (status = 404, description = "Instance not found"),
        (status = 403, description = "Access denied")
    )
)]
pub async fn export_instance(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<crate::auth::AuthUser>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    
    // Get user's current organization (required for multi-tenant scoping)
    let user_org_id = match user.current_organization_id {
        Some(oid) => oid,
        None => {
            return (StatusCode::BAD_REQUEST, "User must be in an organization to access instances").into_response();
        }
    };
    
    // Log access attempt
    let log_id = simple_logger::log_action_with_metadata(
        &state.db,
        "EXPORT_INSTANCE",
        "in_progress",
        Some(id),
        None,
        Some(serde_json::json!({
            "user_id": user.user_id,
            "user_org_id": user_org_id,
            "instance_id": id
        })),
    )
    .await
    .ok();
    
    // Fetch instance (same query as get_instance)
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
          AND i.organization_id = $2
        LIMIT 1
        "#
    )
    .bind(id)
    .bind(user_org_id)
    .fetch_optional(&state.db)
    .await;

    let instance = match row {
        Ok(Some(mut inst)) => {
            // Enrich instance with progress percentage
            progress::enrich_instances_with_progress(&state.db, std::slice::from_mut(&mut inst))
                .await;
            inst
        }
        Ok(None) => {
            if let Some(lid) = log_id {
                let duration_ms = start.elapsed().as_millis() as i32;
                let _ = simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    lid,
                    "failed",
                    duration_ms,
                    Some("Instance not found or access denied"),
                    Some(serde_json::json!({
                        "reason": "not_found_or_access_denied"
                    })),
                )
                .await;
            }
            return (StatusCode::NOT_FOUND, "Instance not found or access denied").into_response();
        }
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    };

    // Fetch storages
    let storages: Vec<InstanceStorageInfo> = sqlx::query_as::<
        Postgres,
        (
            uuid::Uuid,
            String,
            Option<String>,
            String,
            i64,
            bool,
            String,
            bool,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<String>,
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

    // Fetch all action logs for this instance
    let action_logs: Vec<(
        Uuid,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i32>,
        chrono::DateTime<chrono::Utc>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<serde_json::Value>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT
            al.id,
            al.action_type,
            al.component,
            al.status,
            p.name as provider_name,
            it.name as instance_type,
            al.error_code,
            al.error_message,
            al.duration_ms,
            al.created_at,
            al.completed_at,
            al.metadata,
            al.instance_status_before,
            al.instance_status_after
        FROM action_logs al
        LEFT JOIN instances i ON i.id = al.instance_id
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
        WHERE al.instance_id = $1
        ORDER BY al.created_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Fetch state transitions
    let state_transitions_raw: Vec<(
        Uuid,
        Option<String>,
        String,
        Option<String>,
        Option<serde_json::Value>,
        chrono::DateTime<chrono::Utc>,
        Option<i64>,
    )> = sqlx::query_as(
        r#"
        SELECT
            id,
            from_status,
            to_status,
            reason,
            metadata,
            created_at,
            EXTRACT(EPOCH FROM (created_at - (SELECT created_at FROM instances WHERE id = $1)))::bigint as elapsed_seconds_since_start
        FROM instance_state_history
        WHERE instance_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let state_transitions: Vec<StateTransition> = state_transitions_raw
        .into_iter()
        .map(|(id, from_status, to_status, reason, metadata, created_at, elapsed_seconds_since_start)| {
            StateTransition {
                id,
                from_status,
                to_status,
                reason,
                metadata,
                created_at,
                elapsed_seconds_since_start: elapsed_seconds_since_start.unwrap_or(0),
            }
        })
        .collect();

    // Convert action logs to enhanced format
    let instance_created_at = instance.created_at;
    let mut enhanced_actions: Vec<EnhancedActionLog> = Vec::new();
    let mut action_type_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

    for (
        log_id,
        action_type,
        component,
        status,
        provider_name,
        instance_type,
        error_code,
        error_message,
        duration_ms,
        created_at,
        completed_at,
        metadata,
        instance_status_before,
        instance_status_after,
    ) in action_logs
    {
        // Count retries
        let count = action_type_counts.entry(action_type.clone()).or_insert(0);
        *count += 1;
        let retry_count = count.saturating_sub(1);

        // Calculate elapsed time
        let elapsed_start = (created_at - instance_created_at).num_seconds();
        let elapsed_completed = completed_at
            .map(|ct| (ct - instance_created_at).num_seconds());

        // Determine phase
        let phase = determine_phase(&action_type, instance_status_after.as_deref());

        // Extract sub-phase
        let sub_phase = extract_sub_phase(&metadata);

        // Check if state transition
        let is_state_transition = instance_status_before.is_some() 
            && instance_status_after.is_some() 
            && instance_status_before != instance_status_after;

        // Validate transition
        let is_valid_transition = if is_state_transition {
            Some(is_valid_state_transition(
                instance_status_before.as_deref(),
                instance_status_after.as_deref().unwrap_or(""),
            ))
        } else {
            None
        };

        // Calculate progress based on action type and status
        let progress_percent = calculate_action_progress_by_type(
            &action_type,
            &status,
            instance_status_after.as_deref(),
        );

        enhanced_actions.push(EnhancedActionLog {
            id: log_id,
            action_type,
            component,
            status,
            provider_name,
            instance_type,
            error_code,
            error_message,
            instance_id: Some(id),
            duration_ms,
            created_at,
            completed_at,
            metadata,
            instance_status_before,
            instance_status_after,
            progress_percent,
            phase,
            sub_phase,
            retry_count: Some(retry_count),
            is_state_transition,
            is_valid_transition,
            elapsed_seconds_since_start: Some(elapsed_start),
            elapsed_seconds_since_start_completed: elapsed_completed,
        });
    }

    // Calculate phase summaries
    let mut phases: Vec<PhaseSummary> = Vec::new();
    let phase_order = vec!["provisioning", "booting", "installing", "starting", "ready", "draining", "terminating", "terminated"];
    
    for phase_name in phase_order {
        let phase_actions: Vec<&EnhancedActionLog> = enhanced_actions
            .iter()
            .filter(|a| a.phase.as_deref() == Some(phase_name))
            .collect();

        if !phase_actions.is_empty() {
            let start_time = phase_actions.first().map(|a| a.created_at);
            let end_time = phase_actions.last().and_then(|a| a.completed_at);
            let duration = start_time.and_then(|st| {
                end_time.map(|et| (et - st).num_seconds())
            });

            let success_count = phase_actions.iter().filter(|a| a.status == "success").count();
            let failed_count = phase_actions.iter().filter(|a| a.status == "failed").count();

            phases.push(PhaseSummary {
                phase: phase_name.to_string(),
                start_time,
                end_time,
                duration_seconds: duration,
                action_count: phase_actions.len(),
                success_count,
                failed_count,
                progress_start: phase_actions.first().and_then(|a| a.progress_percent),
                progress_end: phase_actions.last().and_then(|a| a.progress_percent),
            });
        }
    }

    // Find invalid transitions
    let invalid_transitions: Vec<String> = enhanced_actions
        .iter()
        .filter_map(|a| {
            if let Some(false) = a.is_valid_transition {
                Some(format!(
                    "{}: {} -> {}",
                    a.action_type,
                    a.instance_status_before.as_deref().unwrap_or("none"),
                    a.instance_status_after.as_deref().unwrap_or("none")
                ))
            } else {
                None
            }
        })
        .collect();

    // Calculate total duration
    let total_duration = enhanced_actions
        .first()
        .and_then(|first| {
            enhanced_actions
                .last()
                .and_then(|last| last.completed_at.map(|ct| (ct - first.created_at).num_seconds()))
        });

    // Count total retries
    let total_retries = enhanced_actions
        .iter()
        .filter(|a| a.retry_count.unwrap_or(0) > 0)
        .count();

    let summary = ExportSummary {
        total_actions: enhanced_actions.len(),
        total_state_transitions: state_transitions.len(),
        phases,
        total_duration_seconds: total_duration,
        invalid_transitions,
        retry_count: total_retries,
        exported_at: chrono::Utc::now(),
    };

    // Log successful export
    if let Some(lid) = log_id {
        let duration_ms = start.elapsed().as_millis() as i32;
        let _ = simple_logger::log_action_complete_with_metadata(
            &state.db,
            lid,
            "success",
            duration_ms,
            None,
            Some(serde_json::json!({
                "total_actions": enhanced_actions.len(),
                "total_transitions": state_transitions.len(),
                "phases_count": summary.phases.len()
            })),
        )
        .await;
    }

    Json(InstanceExportResponse {
        instance,
        storages,
        actions: enhanced_actions,
        state_transitions,
        summary,
    })
    .into_response()
}
