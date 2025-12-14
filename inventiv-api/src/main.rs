use axum::{
    extract::{State, Path},
    routing::{get, post, delete},
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
        .route("/instances/:id", delete(terminate_instance))
        .route("/action_logs", get(list_action_logs))
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
    pub zone_id: uuid::Uuid,
    pub instance_type_id: uuid::Uuid,
    pub status: String,
    pub ip_address: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    
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
    zone: String,
    instance_type: String,
}

#[derive(Serialize, utoipa::ToSchema)]
struct DeploymentResponse {
    status: String,
    instance_id: String,  // Renamed from deployment_id for clarity
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
) -> Json<DeploymentResponse> {
    let start = std::time::Instant::now();
    let instance_id_uuid = uuid::Uuid::new_v4();  // Create UUID first
    let instance_id = instance_id_uuid.to_string();
    
    // LOG 1: REQUEST_CREATE (changed from CREATE_INSTANCE)
    let log_id = simple_logger::log_action_with_metadata(
        &state.db,
        "REQUEST_CREATE",           // ✅ Step 1: Request accepted
        "in_progress",
        Some(instance_id_uuid),     // ✅ Now has instance_id
        None,
        Some(serde_json::json!({
            "zone": payload.zone,
            "instance_type": payload.instance_type,
        })),
    ).await.ok();
    
    println!("🚀 New Instance Creation Request: {}", instance_id);

    // Publish Event to Redis
    let event_payload = serde_json::json!({
        "type": "CMD:PROVISION",
        "instance_id": instance_id,
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

                    Json(DeploymentResponse {
                        status: "accepted".to_string(),
                        instance_id,
                    })
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
                            Some(serde_json::json!({"redis_published": false, "event_type": "CMD:PROVISION"})),
                        ).await.ok();
                    }
                    Json(DeploymentResponse {
                        status: "failed".to_string(),
                        instance_id,
                    })
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
                    Some(serde_json::json!({"redis_published": false, "event_type": "CMD:PROVISION"})),
                ).await.ok();
            }
            Json(DeploymentResponse {
                status: "failed".to_string(),
                instance_id,
            })
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
            i.status::text as status, 
            i.ip_address::text as ip_address, 
            i.created_at,
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
        "UPDATE instances SET is_archived = true WHERE id = $1 AND status = 'terminated'"
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
    
    // 1. Immediately update status to 'terminating' in DB
    let update_result = sqlx::query(
        "UPDATE instances SET status = 'terminating' WHERE id = $1 AND status != 'terminated'"
    )
    .bind(id)
    .execute(&state.db)
    .await;
    
    match update_result {
        Ok(result) if result.rows_affected() > 0 => {
            println!("✅ Instance {} status set to 'terminating'", id);
        }
        Ok(_) => {
            println!("⚠️  Instance {} not found or already terminated", id);
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                simple_logger::log_action_complete(&state.db, log_id, "failed", duration, Some("Instance not found")).await.ok();
            }
            return (StatusCode::NOT_FOUND, "Instance not found or already terminated").into_response();
        }
        Err(e) => {
            println!("❌ Failed to update instance status: {:?}", e);
            if let Some(log_id) = log_id {
                let duration = start.elapsed().as_millis() as i32;
                let msg = format!("Database error: {:?}", e);
                simple_logger::log_action_complete(&state.db, log_id, "failed", duration, Some(&msg)).await.ok();
            }
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
        }
    }

    // 2. Send termination event to orchestrator (async)
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
            error_message, instance_id, duration_ms, created_at, metadata
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
