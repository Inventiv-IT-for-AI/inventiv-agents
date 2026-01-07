use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::app::state::AppState;
use crate::simple_logger;
use redis::AsyncCommands;

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct DeploymentRequest {
    /// Preferred way to select provider (stable): e.g. "scaleway", "mock"
    pub provider_code: Option<String>,
    /// Backward-compat (deprecated): provider UUID
    pub provider_id: Option<uuid::Uuid>,
    pub zone: String,
    pub instance_type: String,
    /// Optional model selection (UUID from /models). If omitted, orchestrator may fallback to env default.
    pub model_id: Option<uuid::Uuid>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DeploymentResponse {
    pub status: String,
    pub instance_id: String, // Renamed from deployment_id for clarity
    pub message: Option<String>,
}

#[utoipa::path(
    post,
    path = "/deployments",
    request_body = DeploymentRequest,
    responses(
        (status = 200, description = "Deployment Accepted", body = DeploymentResponse)
    )
)]
pub async fn create_deployment(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeploymentRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let instance_id_uuid = uuid::Uuid::new_v4(); // Create UUID first
    let instance_id = instance_id_uuid.to_string();

    let requested_provider_code: Option<String> = payload
        .provider_code
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());

    // Resolve provider UUID from provider_code if provided (preferred).
    // If resolution fails we still insert an instance row (traceability), but validation will fail.
    let provider_id_resolved: Option<uuid::Uuid> = if let Some(pid) = payload.provider_id {
        Some(pid)
    } else if let Some(code) = requested_provider_code.as_deref() {
        sqlx::query_scalar("SELECT id FROM providers WHERE code = $1 LIMIT 1")
            .bind(code)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None)
    } else {
        // No provider specified -> default to provider code "scaleway"
        // (no hardcoded UUIDs; seed controls the actual id)
        sqlx::query_scalar("SELECT id FROM providers WHERE code = 'scaleway' LIMIT 1")
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None)
    };

    let provider_id = match provider_id_resolved {
        Some(id) => id,
        None => {
            // Can't resolve provider -> fail early (but still keep instance row traceable).
            // We'll insert with a dummy provider_id? Not possible due FK, so we must stop here.
            return (
                StatusCode::BAD_REQUEST,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some(
                        "Unknown provider (provider_code/provider_id not found)".to_string(),
                    ),
                }),
            )
                .into_response();
        }
    };

    // We want a durable instance_id from the very first request, even when validation fails.
    // So we insert the instance row first (zone/type can be NULL), then all errors can be logged with instance_id.
    //
    // If this ever collides (extremely unlikely), we return 409 so devs notice immediately.
    let insert_initial = sqlx::query(
        "INSERT INTO instances (id, provider_id, zone_id, instance_type_id, status, created_at, gpu_profile)
         VALUES ($1, $2, NULL, NULL, 'provisioning', NOW(), '{}')"
    )
    .bind(instance_id_uuid)
    .bind(provider_id)
    .execute(&state.db)
    .await;

    if let Err(e) = insert_initial {
        // If duplicate key, surface loudly to detect any upstream bug.
        let _msg = format!("Failed to create initial instance id: {:?}", e);
        let is_unique_violation = matches!(e, sqlx::Error::Database(ref db_err) if db_err.code().as_deref() == Some("23505"));

        return (
            if is_unique_violation {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(if is_unique_violation {
                    "Instance id collision (duplicate primary key)".to_string()
                } else {
                    "Database error while creating initial instance id".to_string()
                }),
            }),
        )
            .into_response();
    }

    // LOG 1: REQUEST_CREATE (request is now traceable by instance_id even if validation fails)
    let log_id = simple_logger::log_action_with_metadata(
        &state.db,
        "REQUEST_CREATE",
        "in_progress",
        Some(instance_id_uuid),
        None,
        Some(serde_json::json!({
            "provider_id": provider_id.to_string(),
            "provider_code": requested_provider_code,
            "zone": payload.zone,
            "instance_type": payload.instance_type,
            "model_id": payload.model_id.map(|m| m.to_string()),
        })),
    )
    .await
    .ok();

    // Basic validation: even if invalid, we keep the instance row + log tied to instance_id.
    if payload.zone.trim().is_empty() || payload.instance_type.trim().is_empty() {
        let msg = "Missing zone or instance_type";
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("MISSING_PARAMS")
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": "MISSING_PARAMS"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(msg.to_string()),
            }),
        )
            .into_response();
    }

    // Model is mandatory: request cannot be created without defining the model to install.
    if payload.model_id.is_none() {
        let msg = "Missing model_id";
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("MISSING_MODEL")
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": "MISSING_MODEL"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(msg.to_string()),
            }),
        )
            .into_response();
    }

    // Provider must exist and be active.
    // If a provider_code was provided but did not resolve, treat as invalid.
    let provider_active: bool = if requested_provider_code.is_some()
        && payload.provider_id.is_none()
        && provider_id_resolved.is_none()
    {
        false
    } else {
        sqlx::query_scalar("SELECT COALESCE(is_active, false) FROM providers WHERE id = $1")
            .bind(provider_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None)
            .unwrap_or(false)
    };

    if !provider_active {
        let msg = "Invalid provider (not found or inactive)";
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("INVALID_PROVIDER")
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": "INVALID_PROVIDER"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some("Invalid provider (not found or inactive)".to_string()),
            }),
        )
            .into_response();
    }

    // Zone must be active AND belong to the provider.
    // After schema hardening, zones.code is UNIQUE per provider (zones.provider_id, zones.code).
    // If the catalog is inconsistent, we fail loudly (no ambiguous fallback).
    let zone_rows: Vec<(uuid::Uuid, bool, bool)> = sqlx::query_as(
        r#"SELECT z.id
                , z.is_active
                , r.is_active
           FROM zones z
           JOIN regions r ON r.id = z.region_id
           WHERE z.code = $1
             AND z.provider_id = $2"#,
    )
    .bind(payload.zone.trim())
    .bind(provider_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let zone_id = match zone_rows.as_slice() {
        &[_, _, ..] => {
            let msg = "Catalog inconsistency: duplicate zone code for provider (expected unique zones.provider_id+code)";
            let _ = sqlx::query(
                "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                 WHERE id=$1"
            )
            .bind(instance_id_uuid)
            .bind("CATALOG_INCONSISTENT")
            .bind(msg)
            .execute(&state.db)
            .await;
            if let Some(id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    id,
                    "failed",
                    duration,
                    Some(msg),
                    Some(serde_json::json!({"error_code": "CATALOG_INCONSISTENT"})),
                )
                .await
                .ok();
            }
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some(msg.to_string()),
                }),
            )
                .into_response();
        }
        [(zid, zact, ract)] => {
            let (zid, zact, ract) = (*zid, *zact, *ract);
            if zact && ract {
                zid
            } else {
                let msg = "Invalid zone (not found, inactive, or does not belong to provider)";
                let _ = sqlx::query(
                    "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                     WHERE id=$1"
                )
                .bind(instance_id_uuid)
                .bind("INVALID_ZONE")
                .bind(msg)
                .execute(&state.db)
                .await;

                if let Some(id) = log_id {
                    let duration = start.elapsed().as_millis() as i32;
                    simple_logger::log_action_complete_with_metadata(
                        &state.db,
                        id,
                        "failed",
                        duration,
                        Some(msg),
                        Some(serde_json::json!({"error_code": "INVALID_ZONE"})),
                    )
                    .await
                    .ok();
                }

                return (
                    StatusCode::BAD_REQUEST,
                    Json(DeploymentResponse {
                        status: "failed".to_string(),
                        instance_id,
                        message: Some(msg.to_string()),
                    }),
                )
                    .into_response();
            }
        }
        [] => {
            let msg = "Invalid zone (not found, inactive, or does not belong to provider)";
            let _ = sqlx::query(
                "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                 WHERE id=$1"
            )
            .bind(instance_id_uuid)
            .bind("INVALID_ZONE")
            .bind(msg)
            .execute(&state.db)
            .await;

            if let Some(id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    id,
                    "failed",
                    duration,
                    Some(msg),
                    Some(serde_json::json!({"error_code": "INVALID_ZONE"})),
                )
                .await
                .ok();
            }

            return (
                StatusCode::BAD_REQUEST,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some(msg.to_string()),
                }),
            )
                .into_response();
        }
    };

    // Instance type must exist, be active, and be available in the zone
    let instance_type_row: Option<(uuid::Uuid, bool)> = sqlx::query_as(
        r#"SELECT it.id, it.is_active
           FROM instance_types it
           JOIN instance_type_zones itz ON itz.instance_type_id = it.id
           WHERE it.code = $1
             AND itz.zone_id = $2
             AND itz.is_available = true"#,
    )
    .bind(payload.instance_type.trim())
    .bind(zone_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let instance_type_id = match instance_type_row {
        Some((itid, itact)) if itact => itid,
        _ => {
            let msg = "Invalid instance_type (not found, inactive, or not available in zone)";
            let _ = sqlx::query(
                "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                 WHERE id=$1"
            )
            .bind(instance_id_uuid)
            .bind("INVALID_INSTANCE_TYPE")
            .bind(msg)
            .execute(&state.db)
            .await;

            if let Some(id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    id,
                    "failed",
                    duration,
                    Some(msg),
                    Some(serde_json::json!({"error_code": "INVALID_INSTANCE_TYPE"})),
                )
                .await
                .ok();
            }

            return (
                StatusCode::BAD_REQUEST,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some(msg.to_string()),
                }),
            )
                .into_response();
        }
    };

    // Model must exist and be active
    let model_id = payload.model_id.unwrap();
    let model_active: bool =
        sqlx::query_scalar("SELECT COALESCE(is_active, false) FROM models WHERE id = $1")
            .bind(model_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None)
            .unwrap_or(false);

    if !model_active {
        let msg = "Invalid model (not found or inactive)";
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("INVALID_MODEL")
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": "INVALID_MODEL"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(msg.to_string()),
            }),
        )
            .into_response();
    }

    // Compatibility check: model must fit on instance type
    let compatible: bool = sqlx::query_scalar("SELECT check_model_instance_compatibility($1, $2)")
        .bind(model_id)
        .bind(instance_type_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(false);

    if !compatible {
        let msg = "Model is not compatible with selected instance type (VRAM requirement exceeds available GPU memory)";
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("INCOMPATIBLE_MODEL_INSTANCE")
        .bind(msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(msg),
                Some(serde_json::json!({"error_code": "INCOMPATIBLE_MODEL_INSTANCE"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::BAD_REQUEST,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some(msg.to_string()),
            }),
        )
            .into_response();
    }

    // Update instance row with validated zone/type/model
    let update_result = sqlx::query(
        "UPDATE instances
         SET zone_id = $2,
             instance_type_id = $3,
             model_id = $4
         WHERE id = $1",
    )
    .bind(instance_id_uuid)
    .bind(zone_id)
    .bind(instance_type_id)
    .bind(model_id)
    .execute(&state.db)
    .await;

    if let Err(e) = update_result {
        let msg = format!("Database error updating instance: {:?}", e);
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind("DB_ERROR")
        .bind(&msg)
        .execute(&state.db)
        .await;

        if let Some(id) = log_id {
            let duration = start.elapsed().as_millis() as i32;
            simple_logger::log_action_complete_with_metadata(
                &state.db,
                id,
                "failed",
                duration,
                Some(&msg),
                Some(serde_json::json!({"error_code": "DB_ERROR"})),
            )
            .await
            .ok();
        }

        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
                message: Some("Database error".to_string()),
            }),
        )
            .into_response();
    }

    // Publish CMD:PROVISION event to orchestrator
    let event = serde_json::json!({
        "type": "CMD:PROVISION",
        "instance_id": instance_id,
        "provider_id": provider_id.to_string(),
        "zone_id": zone_id.to_string(),
        "instance_type_id": instance_type_id.to_string(),
        "model_id": model_id.to_string(),
        "correlation_id": log_id.map(|id| id.to_string()),
    })
    .to_string();

    match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            match conn
                .publish::<_, _, ()>("orchestrator_events", &event)
                .await
            {
                Ok(_) => {
                    if let Some(id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            id,
                            "success",
                            duration,
                            None,
                            Some(serde_json::json!({"redis_published": true, "event_type": "CMD:PROVISION"})),
                        )
                        .await
                        .ok();
                    }
                    (
                        StatusCode::OK,
                        Json(DeploymentResponse {
                            status: "accepted".to_string(),
                            instance_id,
                            message: Some("Deployment accepted".to_string()),
                        }),
                    )
                        .into_response()
                }
                Err(e) => {
                    let error_msg = format!("Failed to publish to Redis: {:?}", e);
                    if let Some(id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            id,
                            "failed",
                            duration,
                            Some(&error_msg),
                            Some(serde_json::json!({"redis_published": false, "event_type": "CMD:PROVISION"})),
                        )
                        .await
                        .ok();
                    }
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(DeploymentResponse {
                            status: "failed".to_string(),
                            instance_id,
                            message: Some("Failed to queue deployment".to_string()),
                        }),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to connect to Redis: {:?}", e);
            if let Some(id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    id,
                    "failed",
                    duration,
                    Some(&error_msg),
                    Some(serde_json::json!({"redis_published": false, "event_type": "CMD:PROVISION"})),
                )
                .await
                .ok();
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DeploymentResponse {
                    status: "failed".to_string(),
                    instance_id,
                    message: Some("Failed to connect to Redis".to_string()),
                }),
            )
                .into_response()
        }
    }
}
