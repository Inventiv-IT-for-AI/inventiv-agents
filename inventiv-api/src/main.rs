use axum::{
    extract::{State, Path},
    routing::{get, post},
    Router, Json,
    http::StatusCode,
    response::IntoResponse,
};
use tower_http::cors::{CorsLayer, Any};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use redis::AsyncCommands;
use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

// Swagger
use utoipa::{OpenApi, IntoParams};
use utoipa_swagger_ui::SwaggerUi;
mod settings; // Module
mod action_logs_search;
mod api_docs;
mod simple_logger;
mod instance_type_zones; // Module for zone associations
 // Simple logger without sqlx macros

// use audit_log::AuditLogger; // Commented out due to DATABASE_URL build issues

#[derive(Clone)]
struct AppState {
    redis_client: redis::Client,
    db: Pool<Postgres>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let client = redis::Client::open(redis_url).unwrap();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to Postgres");

    // Run migrations (source of truth is /migrations at workspace root)
    // Safe to run on startup; sqlx uses the _sqlx_migrations table + lock.
    // Note: migrations are embedded at compile-time; code change forces rebuild.
    sqlx::migrate!("../sqlx-migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let state = Arc::new(AppState {
        redis_client: client,
        db: pool,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", api_docs::ApiDoc::openapi()))
        .route("/", get(root))
        .route("/deployments", post(create_deployment))
        // NEW READ ENDPOINTS
        .route("/instances", get(list_instances))
        .route("/instances/:id/archive", axum::routing::put(archive_instance))
        .route("/instances/:id", get(get_instance).delete(terminate_instance))
        .route("/action_logs", get(list_action_logs))
        .route("/action_logs/search", get(action_logs_search::search_action_logs))
        .route("/action_types", get(list_action_types))
        .route("/reconcile", post(manual_reconcile_trigger))
        .route("/catalog/sync", post(manual_catalog_sync_trigger))
        // SETTINGS ENDPOINTS
        .route("/providers", get(settings::list_providers))
        .route("/providers/:id", axum::routing::put(settings::update_provider))
        .route("/regions", get(settings::list_regions))
        .route("/regions/:id", axum::routing::put(settings::update_region))
        .route("/zones", get(settings::list_zones))
        .route("/zones/:id", axum::routing::put(settings::update_zone))
        .route("/instance_types", get(settings::list_instance_types))
        .route("/instance_types/:id", axum::routing::put(settings::update_instance_type))
        // INSTANCE TYPE ZONE ASSOCIATIONS
        .route("/instance_types/:id/zones", get(instance_type_zones::list_instance_type_zones))
        .route("/instance_types/:id/zones", axum::routing::put(instance_type_zones::associate_zones_to_instance_type))
        .route("/zones/:zone_id/instance_types", get(instance_type_zones::list_instance_types_for_zone))
        .layer(cors)  // Apply CORS to ALL routes
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8003));
    println!("Backend listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
pub struct InstanceResponse {
    pub id: uuid::Uuid,
    pub provider_id: uuid::Uuid,
    pub zone_id: Option<uuid::Uuid>,
    pub instance_type_id: Option<uuid::Uuid>,
    pub provider_instance_id: Option<String>,
    pub status: String,
    pub ip_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub terminated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_health_check: Option<chrono::DateTime<chrono::Utc>>,
    pub last_reconciliation: Option<chrono::DateTime<chrono::Utc>>,
    pub health_check_failures: Option<i32>,
    pub deletion_reason: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    
    // Joined Fields
    pub provider_name: String,
    pub region: String,
    pub zone: String,
    pub instance_type: String,
    pub gpu_vram: Option<i32>,
    pub gpu_count: Option<i32>,     // NEW: Distinct GPU count
    pub cost_per_hour: Option<f64>,
    pub total_cost: Option<f64>,
    pub is_archived: bool,
    pub deleted_by_provider: Option<bool>,
}

#[derive(Deserialize, IntoParams)]
pub struct ListInstanceParams {
    pub archived: Option<bool>,
}

async fn root() -> &'static str {
    "Inventiv Backend API (Product Plane) - CQRS Enabled"
}

// ... (DeploymentRequest structs) ...

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
struct DeploymentRequest {
    provider_id: Option<uuid::Uuid>,
    zone: String,
    instance_type: String,
}

#[derive(Serialize, utoipa::ToSchema)]
struct DeploymentResponse {
    status: String,
    instance_id: String,  // Renamed from deployment_id for clarity
    message: Option<String>,
}

// COMMAND : CREATE DEPLOYMENT
#[utoipa::path(
    post,
    path = "/deployments",
    request_body = DeploymentRequest,
    responses(
        (status = 200, description = "Deployment Accepted", body = DeploymentResponse)
    )
)]
async fn create_deployment(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeploymentRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let instance_id_uuid = uuid::Uuid::new_v4();  // Create UUID first
    let instance_id = instance_id_uuid.to_string();

    // If provider_id isn't sent (older clients), default to Scaleway.
    let default_provider_id =
        uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(); // scaleway
    let provider_id = payload.provider_id.unwrap_or(default_provider_id);

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
            if is_unique_violation { StatusCode::CONFLICT } else { StatusCode::INTERNAL_SERVER_ERROR },
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
            "zone": payload.zone,
            "instance_type": payload.instance_type,
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

    // Provider must exist and be active
    let provider_active: bool = sqlx::query_scalar("SELECT COALESCE(is_active, false) FROM providers WHERE id = $1")
        .bind(provider_id)
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None)
        .unwrap_or(false);

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

    // Zone must be active AND belong to the provider via its region
    let zone_row: Option<(uuid::Uuid, bool, bool)> = sqlx::query_as(
        r#"SELECT z.id
                , z.is_active
                , r.is_active
           FROM zones z
           JOIN regions r ON r.id = z.region_id
           WHERE z.code = $1
             AND r.provider_id = $2"#
    )
    .bind(&payload.zone)
    .bind(provider_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    // Instance type must be active and belong to the provider
    let type_row: Option<(uuid::Uuid, bool)> = sqlx::query_as(
        r#"SELECT it.id
                , it.is_active
           FROM instance_types it
           WHERE it.code = $1
             AND it.provider_id = $2"#
    )
    .bind(&payload.instance_type)
    .bind(provider_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    // Persist resolved ids (even if inactive) to keep request traceable in instances table
    let resolved_zone_id: Option<uuid::Uuid> = zone_row.map(|(id, _z_active, _r_active)| id);
    let resolved_type_id: Option<uuid::Uuid> = type_row.map(|(id, _active)| id);
    let _ = sqlx::query("UPDATE instances SET zone_id=$2, instance_type_id=$3 WHERE id=$1")
        .bind(instance_id_uuid)
        .bind(resolved_zone_id)
        .bind(resolved_type_id)
        .execute(&state.db)
        .await;

    // Validation
    let mut validation_error: Option<(&'static str, &'static str)> = None;
    match zone_row {
        None => validation_error = Some(("INVALID_ZONE", "Invalid zone (not found for provider)")),
        Some((_id, z_active, r_active)) if !z_active || !r_active => {
            validation_error = Some(("INACTIVE_ZONE", "Zone is inactive"))
        }
        _ => {}
    }
    match type_row {
        None => validation_error = Some(("INVALID_INSTANCE_TYPE", "Invalid instance type (not found for provider)")),
        Some((_id, active)) if !active => validation_error = Some(("INACTIVE_INSTANCE_TYPE", "Instance type is inactive")),
        _ => {}
    }

    if let Some((code, msg)) = validation_error {
        let _ = sqlx::query(
            "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
             WHERE id=$1"
        )
        .bind(instance_id_uuid)
        .bind(code)
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
                Some(serde_json::json!({"error_code": code})),
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
    
    println!("🚀 New Instance Creation Request: {}", instance_id);

    // Publish Event to Redis
    let event_payload = serde_json::json!({
        "type": "CMD:PROVISION",
        "instance_id": instance_id,
        "provider_id": provider_id.to_string(),
        "zone": payload.zone,
        "instance_type": payload.instance_type,
        "correlation_id": log_id.map(|id| id.to_string()),
    }).to_string();

    println!("📤 Publishing provisioning event to Redis: {}", event_payload);

    match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            match conn.publish::<_, _, ()>("orchestrator_events", &event_payload).await {
                Ok(_) => {
                    println!("✅ Provisioning event published successfully");
                    // Log completion
                    if let Some(id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            id,
                            "success",
                            duration,
                            None,
                            Some(serde_json::json!({"redis_published": true, "event_type": "CMD:PROVISION"})),
                        ).await.ok();
                    }

                    (
                        StatusCode::ACCEPTED,
                        Json(DeploymentResponse {
                        status: "accepted".to_string(),
                        instance_id,
                        message: None,
                        }),
                    )
                        .into_response()
                }
                Err(e) => {
                    let error_msg = format!("Failed to publish to Redis: {:?}", e);
                    println!("❌ {}", error_msg);
                    if let Some(id) = log_id {
                        let duration = start.elapsed().as_millis() as i32;
                        simple_logger::log_action_complete_with_metadata(
                            &state.db,
                            id,
                            "failed",
                            duration,
                            Some(&error_msg),
                            Some(serde_json::json!({"redis_published": false, "event_type": "CMD:PROVISION", "error_code": "REDIS_PUBLISH_FAILED"})),
                        ).await.ok();
                    }
                    let _ = sqlx::query(
                        "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                         WHERE id=$1"
                    )
                    .bind(instance_id_uuid)
                    .bind("REDIS_PUBLISH_FAILED")
                    .bind(&error_msg)
                    .execute(&state.db)
                    .await;
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(DeploymentResponse {
                            status: "failed".to_string(),
                            instance_id,
                            message: Some("Failed to queue provisioning event".to_string()),
                        }),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to connect to Redis: {:?}", e);
            println!("❌ {}", error_msg);
            if let Some(id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete_with_metadata(
                    &state.db,
                    id,
                    "failed",
                    duration,
                    Some(&error_msg),
                    Some(serde_json::json!({"redis_published": false, "event_type": "CMD:PROVISION", "error_code": "REDIS_CONNECT_FAILED"})),
                ).await.ok();
            }
            let _ = sqlx::query(
                "UPDATE instances SET status='provisioning_failed', error_code=$2, error_message=$3, failed_at=NOW()
                 WHERE id=$1"
            )
            .bind(instance_id_uuid)
            .bind("REDIS_CONNECT_FAILED")
            .bind(&error_msg)
            .execute(&state.db)
            .await;
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

/// POST /reconcile - Trigger manual reconciliation
#[utoipa::path(
    post,
    path = "/reconcile",
    responses(
        (status = 200, description = "Reconciliation triggered", body = serde_json::Value),
        (status = 500, description = "Failed to trigger reconciliation", body = serde_json::Value)
    )
)]
async fn manual_reconcile_trigger(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    println!("🔍 Manual reconciliation triggered via API");

    // Publish Redis event for orchestrator
    let event_payload = serde_json::json!({
        "type": "CMD:RECONCILE"
    }).to_string();

    let mut conn = state.redis_client.get_multiplexed_async_connection().await.unwrap();
    // Use turbofish to specify return type as unit ()
    match conn.publish::<_, _, ()>("orchestrator_events", &event_payload).await {
        Ok(_) => {
            Json(json!({
                "status": "triggered",
                "message": "Reconciliation task has been triggered"
            }))
        }
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
async fn manual_catalog_sync_trigger(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    println!("🔄 Catalog Sync triggered via API");

    // Publish Redis event for orchestrator
    let event_payload = serde_json::json!({
        "type": "CMD:SYNC_CATALOG"
    }).to_string();

    let mut conn = state.redis_client.get_multiplexed_async_connection().await.unwrap();
    // Use turbofish to specify return type as unit ()
    match conn.publish::<_, _, ()>("orchestrator_events", &event_payload).await {
        Ok(_) => {
            Json(json!({
                "status": "triggered",
                "message": "Catalog Sync task has been triggered"
            }))
        }
        Err(e) => {
            eprintln!("Failed to publish sync event: {:?}", e);
            Json(json!({
                "status": "error",
                "message": format!("Failed to trigger sync: {:?}", e)
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/instances",
    params(ListInstanceParams),
    responses(
        (status = 200, description = "List all instances with details", body = Vec<InstanceResponse>)
    )
)]
async fn list_instances(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<ListInstanceParams>,
) -> Json<Vec<InstanceResponse>> {
    let show_archived = params.archived.unwrap_or(false);

    let instances = sqlx::query_as::<Postgres, InstanceResponse>(
        r#"
        SELECT 
            i.id, i.provider_id, i.zone_id, i.instance_type_id,
            i.provider_instance_id::text as provider_instance_id,
            i.status::text as status, 
            i.ip_address::text as ip_address, 
            i.created_at,
            i.terminated_at,
            i.last_health_check,
            (i.last_reconciliation AT TIME ZONE 'UTC') as last_reconciliation,
            i.health_check_failures,
            i.deletion_reason,
            i.error_code,
            i.error_message,
            i.is_archived,
            i.deleted_by_provider,
            COALESCE(p.name, 'Unknown Provider') as provider_name,
            COALESCE(z.name, 'Unknown Zone') as zone,
            COALESCE(r.name, 'Unknown Region') as region,
            COALESCE(it.name, 'Unknown Type') as instance_type,
            it.vram_per_gpu_gb as gpu_vram,
            COALESCE(it.gpu_count, it.n_gpu) as gpu_count,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            (EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8) as total_cost
        FROM instances i
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN zones z ON i.zone_id = z.id
        LEFT JOIN regions r ON z.region_id = r.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
        WHERE i.is_archived = $1
        ORDER BY i.created_at DESC
        "#
    )
    .bind(show_archived)
    .fetch_all(&state.db)
    .await
    .unwrap_or(vec![]);

    Json(instances)
}

#[utoipa::path(
    get,
    path = "/instances/{id}",
    params(
        ("id" = Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 200, description = "Instance details", body = InstanceResponse),
        (status = 404, description = "Instance not found")
    )
)]
async fn get_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<Postgres, InstanceResponse>(
        r#"
        SELECT 
            i.id, i.provider_id, i.zone_id, i.instance_type_id,
            i.provider_instance_id::text as provider_instance_id,
            i.status::text as status, 
            i.ip_address::text as ip_address, 
            i.created_at,
            i.terminated_at,
            i.last_health_check,
            (i.last_reconciliation AT TIME ZONE 'UTC') as last_reconciliation,
            i.health_check_failures,
            i.deletion_reason,
            i.error_code,
            i.error_message,
            i.is_archived,
            i.deleted_by_provider,
            COALESCE(p.name, 'Unknown Provider') as provider_name,
            COALESCE(z.name, 'Unknown Zone') as zone,
            COALESCE(r.name, 'Unknown Region') as region,
            COALESCE(it.name, 'Unknown Type') as instance_type,
            it.vram_per_gpu_gb as gpu_vram,
            COALESCE(it.gpu_count, it.n_gpu) as gpu_count,
            cast(it.cost_per_hour as float8) as cost_per_hour,
            (EXTRACT(EPOCH FROM (COALESCE(i.terminated_at, NOW()) - i.created_at)) / 3600.0) * cast(it.cost_per_hour as float8) as total_cost
        FROM instances i
        LEFT JOIN providers p ON i.provider_id = p.id
        LEFT JOIN zones z ON i.zone_id = z.id
        LEFT JOIN regions r ON z.region_id = r.id
        LEFT JOIN instance_types it ON i.instance_type_id = it.id
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
        Some(inst) => Json(inst).into_response(),
        None => (StatusCode::NOT_FOUND, "Instance not found").into_response(),
    }
}

// Archive endpoint (logged version below)
// COMMAND : ARCHIVE INSTANCE
#[utoipa::path(
    put,
    path = "/instances/{id}/archive",
    params(
        ("id" = Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 200, description = "Instance Archived"),
        (status = 400, description = "Bad Request"),
        (status = 500, description = "Server Error")
    )
)]
async fn archive_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Log start of archive action
    let start = std::time::Instant::now();
    let log_id = simple_logger::log_action(
        &state.db,
        "ARCHIVE_INSTANCE",
        "in_progress",
        Some(id),
        None,
    )
    .await
    .ok();

    let result = sqlx::query(
        "UPDATE instances
         SET is_archived = true,
             status = 'archived'
         WHERE id = $1
           AND status IN ('terminated', 'archived')"
    )
    .bind(id)
    .execute(&state.db)
    .await;

    let response = match result {
        Ok(r) if r.rows_affected() > 0 => (StatusCode::OK, "Instance Archived"),
        Ok(_) => (StatusCode::BAD_REQUEST, "Instance not found or not terminated"),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database Error"),
    };

    // Log completion
    if let Some(lid) = log_id {
        let duration = start.elapsed().as_millis() as i32;
        let status_str = match response.0 {
            StatusCode::OK => "success",
            _ => "failed",
        };
        let err_msg = if response.0 == StatusCode::OK { None } else { Some(response.1) };
        simple_logger::log_action_complete(&state.db, lid, status_str, duration, err_msg).await.ok();
    }

    response.into_response()
}

// COMMAND : TERMINATE INSTANCE
#[utoipa::path(
    delete,
    path = "/instances/{id}",
    params(
        ("id" = Uuid, Path, description = "Instance Database UUID")
    ),
    responses(
        (status = 202, description = "Termination Accepted")
    )
)]
async fn terminate_instance(
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
    ).await.ok();
    
    println!("🗑️ Termination Request: {}", id);

    // 1. Fetch instance so we can handle edge-cases safely (no provider resource, missing zone, etc.)
    let instance_row: Option<(Option<String>, Option<uuid::Uuid>, String)> = sqlx::query_as(
        "SELECT provider_instance_id::text, zone_id, status::text FROM instances WHERE id = $1"
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
            simple_logger::log_action_complete(&state.db, log_id, "failed", duration, Some("Instance not found"))
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
    if provider_instance_id_opt.as_deref().unwrap_or("").is_empty() || zone_id_opt.is_none() {
        let _ = sqlx::query(
            "UPDATE instances
             SET status='terminated',
                 terminated_at = COALESCE(terminated_at, NOW()),
                 deletion_reason = COALESCE(deletion_reason, 'no_provider_resource')
             WHERE id=$1 AND status != 'terminated'"
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

    // 2. Update status to 'terminating' in DB (provider resource exists, orchestrator will delete it)
    let update_result = sqlx::query(
        "UPDATE instances SET status = 'terminating' WHERE id = $1 AND status != 'terminated'"
    )
    .bind(id)
    .execute(&state.db)
    .await;

    match update_result {
        Ok(result) if result.rows_affected() > 0 => println!("✅ Instance {} status set to 'terminating'", id),
        Ok(_) => {
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete(&state.db, log_id, "failed", duration, Some("Instance not found"))
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
                simple_logger::log_action_complete(&state.db, log_id, "failed", duration, Some(&msg))
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
    }).to_string();

    println!("📤 Publishing termination event to Redis: {}", event);
    
    match state.redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            match conn.publish::<_, _, ()>("orchestrator_events", &event).await {
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
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to queue termination").into_response()
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
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to queue termination").into_response()
        }
    }
}

// ============================================================================
// ACTION LOGS API
// ============================================================================

use axum::extract::Query;

#[derive(Deserialize, IntoParams)]
struct ActionLogQuery {
    instance_id: Option<uuid::Uuid>,
    component: Option<String>,
    status: Option<String>,
    action_type: Option<String>,
    limit: Option<i32>,
}

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
struct ActionLogResponse {
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
async fn list_action_logs(
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
         LIMIT $5"
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

// ============================================================================
// ACTION TYPES (UI CATALOG)
// ============================================================================

#[derive(Serialize, sqlx::FromRow, utoipa::ToSchema)]
struct ActionTypeResponse {
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
async fn list_action_types(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<ActionTypeResponse>> {
    let rows = sqlx::query_as::<Postgres, ActionTypeResponse>(
        "SELECT code, label, icon, color_class, category, is_active
         FROM action_types
         WHERE is_active = true
         ORDER BY category NULLS LAST, code ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(rows)
}
